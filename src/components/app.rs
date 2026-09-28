// Copyright 2023 System76 <info@system76.com>
// Copyright 2026 Daniel Probst <daenuprobst@gmail.com>
// SPDX-License-Identifier: GPL-3.0-only

use cctk::{
    sctk::reexports::{
        calloop::channel::SyncSender,
        protocols::ext::workspace::v1::client::ext_workspace_handle_v1::{
            self, ExtWorkspaceHandleV1,
        },
    },
    workspace::Workspace,
};
use cosmic::{
    Element, Task, Theme, app,
    applet::{cosmic_panel_config::PanelAnchor, padded_control},
    cosmic_config::{self, CosmicConfigEntry},
    iced::core::{Background, Border, Color},
    iced::{
        Alignment,
        Event::Mouse,
        Length, Limits, Subscription, event,
        mouse::{self, ScrollDelta},
        platform_specific::shell::wayland::commands::popup::destroy_popup,
        widget::{button, column, row},
        window,
    },
    scroll::DiscreteScrollState,
    surface,
    widget::{Id, autosize, container, divider, mouse_area, slider, space, text, toggler},
};

use crate::{
    config::{self, ActiveColor, WorkspacesConfig},
    fl,
    wayland::WorkspaceEvent,
    wayland_subscription::{WorkspacesUpdate, workspaces},
};

use std::{process::Command as ShellCommand, sync::LazyLock, time::Duration};

static AUTOSIZE_MAIN_ID: LazyLock<Id> = LazyLock::new(|| Id::new("autosize-main"));

const SCROLL_RATE_LIMIT: Duration = Duration::from_millis(200);

/// Color of a workspace number or dot.
fn label_color(
    config: WorkspacesConfig,
    state: ext_workspace_handle_v1::State,
    theme: &Theme,
) -> Color {
    if state.contains(ext_workspace_handle_v1::State::Active) {
        config.active_color.color(theme)
    } else if state.contains(ext_workspace_handle_v1::State::Urgent) {
        theme.cosmic().destructive_button.base.into()
    } else {
        Color::from(theme.current_container().component.on).scale_alpha(config.inactive_opacity)
    }
}

/// Black or white, whichever reads better on `color`, with its opacity.
fn on_color(color: Color) -> Color {
    let luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    let on = if luminance > 0.5 {
        Color::BLACK
    } else {
        Color::WHITE
    };
    Color { a: color.a, ..on }
}

pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<LowkeyWorkspacesApplet>(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Row,
    Column,
}

struct LowkeyWorkspacesApplet {
    core: cosmic::app::Core,
    workspaces: Vec<Workspace>,
    workspace_tx: Option<SyncSender<WorkspaceEvent>>,
    layout: Layout,
    scroll: DiscreteScrollState,
    config: WorkspacesConfig,
    popup: Option<window::Id>,
}

impl LowkeyWorkspacesApplet {
    /// Padding on each side of a workspace indicator along the panel.
    fn spacing(&self) -> u16 {
        self.config
            .spacing
            .unwrap_or(self.core.applet.suggested_padding(true).1)
    }

    fn circle_size(&self) -> u16 {
        self.config
            .circle_size
            .unwrap_or(self.core.applet.suggested_size(true).0 + 4)
    }

    /// returns the index of the workspace button after which which must be moved to a popup
    /// if it exists.
    fn popup_index(&self) -> Option<usize> {
        let mut index = None;
        let Some(max_major_axis_len) = self.core.applet.suggested_bounds.as_ref().map(|c| {
            // if we have a configure for width and height, we're in a overflow popup
            match self.core.applet.anchor {
                PanelAnchor::Top | PanelAnchor::Bottom => c.width as u32,
                PanelAnchor::Left | PanelAnchor::Right => c.height as u32,
            }
        }) else {
            return index;
        };
        let button_total_size = self.core.applet.suggested_size(true).0 + self.spacing() * 2 + 4;
        let btn_count = max_major_axis_len / button_total_size as u32;
        if btn_count >= self.workspaces.len() as u32 {
            index = None;
        } else {
            index = Some((btn_count as usize).min(self.workspaces.len()));
        }
        index
    }
}

#[derive(Debug, Clone)]
enum Message {
    WorkspaceUpdate(WorkspacesUpdate),
    WorkspacePressed(ExtWorkspaceHandleV1),
    WheelScrolled(ScrollDelta),
    WorkspaceOverview,
    Surface(surface::Action<Message>),
    ToggleSettings,
    CloseRequested(window::Id),
    ConfigChanged(WorkspacesConfig),
    SetConfig(WorkspacesConfig),
}

impl cosmic::Application for LowkeyWorkspacesApplet {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = config::APP_ID;

    fn init(core: cosmic::app::Core, _flags: Self::Flags) -> (Self, app::Task<Self::Message>) {
        (
            Self {
                layout: match &core.applet.anchor {
                    PanelAnchor::Left | PanelAnchor::Right => Layout::Column,
                    PanelAnchor::Top | PanelAnchor::Bottom => Layout::Row,
                },
                core,
                workspaces: Vec::new(),
                workspace_tx: Option::default(),
                scroll: DiscreteScrollState::default().rate_limit(Some(SCROLL_RATE_LIMIT)),
                config: cosmic_config::Config::new(config::APP_ID, WorkspacesConfig::VERSION)
                    .map(|c| WorkspacesConfig::get_entry(&c).unwrap_or_else(|(_, c)| c))
                    .unwrap_or_default(),
                popup: None,
            },
            Task::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn update(&mut self, message: Self::Message) -> app::Task<Self::Message> {
        match message {
            Message::WorkspaceUpdate(msg) => match msg {
                WorkspacesUpdate::Workspaces(mut list) => {
                    list.retain(|w| !w.state.contains(ext_workspace_handle_v1::State::Hidden));
                    list.sort_by(|w1, w2| w1.coordinates.cmp(&w2.coordinates));
                    self.workspaces = list;
                }
                WorkspacesUpdate::Started(tx) => {
                    self.workspace_tx.replace(tx);
                }
                WorkspacesUpdate::Errored => {
                    // TODO
                }
            },
            Message::WorkspacePressed(id) => {
                if let Some(tx) = self.workspace_tx.as_mut() {
                    let _ = tx.try_send(WorkspaceEvent::Activate(id));
                }
            }
            Message::WheelScrolled(delta) => {
                let discrete_delta = self.scroll.update(delta);
                if discrete_delta.y != 0 {
                    if let Some(w_i) = self
                        .workspaces
                        .iter()
                        .position(|w| w.state.contains(ext_workspace_handle_v1::State::Active))
                    {
                        let d_i = (w_i as isize - discrete_delta.y)
                            .rem_euclid(self.workspaces.len() as isize)
                            as usize;

                        if let Some(tx) = self.workspace_tx.as_mut() {
                            let _ = tx.try_send(WorkspaceEvent::Activate(
                                self.workspaces[d_i].handle.clone(),
                            ));
                        }
                    }
                }
            }
            Message::WorkspaceOverview => {
                let _ = ShellCommand::new("cosmic-workspaces").spawn();
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Surface(a));
            }
            Message::ToggleSettings => {
                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                }
                return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    |app: &mut Self| {
                        let new_id = window::Id::unique();
                        app.popup = Some(new_id);
                        let mut popup_settings = app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        );
                        popup_settings.positioner.size = None;
                        popup_settings
                    },
                    None,
                ));
            }
            Message::CloseRequested(id) => {
                if Some(id) == self.popup {
                    self.popup = None;
                }
            }
            Message::ConfigChanged(c) => {
                self.config = c;
            }
            Message::SetConfig(c) => {
                self.config = c;
                if let Ok(helper) =
                    cosmic_config::Config::new(config::APP_ID, WorkspacesConfig::VERSION)
                    && let Err(err) = self.config.write_entry(&helper)
                {
                    tracing::error!(?err, "Error writing config");
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if self.workspaces.is_empty() {
            return row![].padding(8).into();
        }
        let horizontal = matches!(
            self.core.applet.anchor,
            PanelAnchor::Top | PanelAnchor::Bottom
        );
        // Outer edges keep the panel padding, only the space between buttons changes.
        let panel_padding = self.core.applet.suggested_padding(true).1;
        let padding = self.spacing().min(panel_padding);
        let gap = 4 + 2 * self.spacing().saturating_sub(panel_padding);
        let edge = panel_padding - padding;
        let circle = self.config.show_circles && !self.config.show_dots;
        let diameter = self.circle_size();
        let mut suggested_total = self.core.applet.suggested_size(true).0 + padding * 2;
        if circle {
            suggested_total = suggested_total.max(diameter);
        }
        let suggested_window_size = self.core.applet.suggested_window_size();
        let popup_index = self.popup_index().unwrap_or(self.workspaces.len());

        let buttons = self.workspaces[..popup_index].iter().map(|w| {
            let config = self.config;
            let hover_config = WorkspacesConfig {
                inactive_opacity: (config.inactive_opacity + 0.2).min(1.0),
                ..config
            };
            let state = w.state;
            let label = if config.show_dots { "●" } else { &w.name };
            let content = self.core.applet.text(label).font(cosmic::font::bold());

            let (width, height) = if self.core.applet.is_horizontal() {
                (suggested_total as f32, suggested_window_size.1.get() as f32)
            } else {
                (suggested_window_size.0.get() as f32, suggested_total as f32)
            };

            let content: Element<_> = if circle {
                container(content).center(diameter).into()
            } else {
                let content = row!(content, space::vertical().height(Length::Fixed(height)))
                    .align_y(Alignment::Center);
                column!(content, space::horizontal().width(Length::Fixed(width)))
                    .align_x(Alignment::Center)
                    .into()
            };

            let btn = button(content)
                .padding(if horizontal {
                    [0, self.core.applet.suggested_padding(true).1]
                } else {
                    [self.core.applet.suggested_padding(true).1, 0]
                })
                .on_press(
                    if w.state.contains(ext_workspace_handle_v1::State::Active) {
                        Message::WorkspaceOverview
                    } else {
                        Message::WorkspacePressed(w.handle.clone())
                    },
                )
                .padding(0);

            let btn = btn.class(
                if w.state.contains(ext_workspace_handle_v1::State::Urgent)
                    && !w.state.contains(ext_workspace_handle_v1::State::Active)
                    && !circle
                {
                    let appearance = |theme: &Theme| {
                        let cosmic = theme.cosmic();
                        button::Style {
                            background: Some(Background::Color(cosmic.palette.neutral_3.into())),
                            border: Border {
                                radius: cosmic.radius_xl().into(),
                                ..Default::default()
                            },
                            border_radius: theme.cosmic().radius_xl().into(),
                            text_color: theme.cosmic().destructive_button.base.into(),
                            ..button::Style::default()
                        }
                    };
                    cosmic::theme::iced::Button::Custom(Box::new(
                        move |theme, status| match status {
                            button::Status::Active => appearance(theme),
                            button::Status::Hovered => button::Style {
                                background: Some(Background::Color(
                                    theme.cosmic().text_button.hover.into(),
                                )),
                                border: Border {
                                    radius: theme.cosmic().radius_xl().into(),
                                    ..Default::default()
                                },
                                ..appearance(theme)
                            },
                            button::Status::Pressed => button::Style {
                                background: Some(Background::Color(
                                    theme.cosmic().text_button.pressed.into(),
                                )),
                                ..appearance(theme)
                            },
                            button::Status::Disabled => appearance(theme),
                        },
                    ))
                } else {
                    let style = move |theme: &Theme, config| {
                        let color = label_color(config, state, theme);
                        let radius = if circle {
                            [diameter as f32 / 2.0; 4]
                        } else {
                            theme.cosmic().radius_xl()
                        };
                        button::Style {
                            background: circle.then_some(Background::Color(color)),
                            border: Border {
                                radius: radius.into(),
                                ..Default::default()
                            },
                            border_radius: radius.into(),
                            text_color: if circle { on_color(color) } else { color },
                            ..button::Style::default()
                        }
                    };
                    let appearance = move |theme: &Theme| style(theme, config);
                    let hovered = move |theme: &Theme| style(theme, hover_config);
                    cosmic::theme::iced::Button::Custom(Box::new(
                        move |theme, status| match status {
                            button::Status::Active => appearance(theme),
                            button::Status::Hovered | button::Status::Pressed if circle => {
                                hovered(theme)
                            }
                            button::Status::Hovered => button::Style {
                                background: Some(Background::Color(
                                    theme.cosmic().text_button.hover.into(),
                                )),
                                border: Border {
                                    radius: theme.cosmic().radius_xl().into(),
                                    ..Default::default()
                                },
                                ..hovered(theme)
                            },
                            button::Status::Pressed => button::Style {
                                background: Some(Background::Color(
                                    theme.cosmic().text_button.pressed.into(),
                                )),
                                ..hovered(theme)
                            },
                            button::Status::Disabled => appearance(theme),
                        },
                    ))
                },
            );

            if circle {
                container(btn).center_x(width).center_y(height).into()
            } else {
                btn.into()
            }
        });
        // TODO if there is a popup_index, create a button with a popup for the remaining workspaces
        // Should it appear on hover or on click?
        let layout_section: Element<_> = match self.layout {
            Layout::Row => row(buttons).spacing(gap).padding([0, edge]).into(),
            Layout::Column => column(buttons).spacing(gap).padding([edge, 0]).into(),
        };
        let mut limits = Limits::NONE.min_width(1.).min_height(1.);
        if let Some(b) = self.core.applet.suggested_bounds {
            if b.width as i32 > 0 {
                limits = limits.max_width(b.width);
            }
            if b.height as i32 > 0 {
                limits = limits.max_height(b.height);
            }
        }

        // Right click opens the settings popup.
        let layout_section = mouse_area(layout_section).on_right_press(Message::ToggleSettings);

        autosize::autosize(
            container(layout_section).padding(0),
            AUTOSIZE_MAIN_ID.clone(),
        )
        .limits(limits)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            workspaces().map(Message::WorkspaceUpdate),
            event::listen_with(|e, _, _| match e {
                Mouse(mouse::Event::WheelScrolled { delta }) => Some(Message::WheelScrolled(delta)),
                _ => None,
            }),
            self.core.watch_config(config::APP_ID).map(|u| {
                for err in u.errors {
                    tracing::error!(?err, "Error watching config");
                }
                Message::ConfigChanged(u.config)
            }),
        ])
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        let config = self.config;
        let swatch = |color: ActiveColor| -> Element<'_, Message> {
            let selected = color == config.active_color;
            button(space::horizontal().width(Length::Fixed(24.0)))
                .height(Length::Fixed(24.0))
                .padding(0)
                .on_press(Message::SetConfig(WorkspacesConfig {
                    active_color: color,
                    ..config
                }))
                .class(cosmic::theme::iced::Button::Custom(Box::new(
                    move |theme, _| button::Style {
                        background: Some(Background::Color(color.color(theme))),
                        border: Border {
                            radius: theme.cosmic().radius_xl().into(),
                            width: if selected { 3.0 } else { 1.0 },
                            color: if selected {
                                theme.current_container().component.on.into()
                            } else {
                                theme.current_container().component.divider.into()
                            },
                        },
                        ..button::Style::default()
                    },
                )))
                .into()
        };
        let swatches = column(
            ActiveColor::ALL
                .chunks(7)
                .map(|chunk| row(chunk.iter().copied().map(swatch)).spacing(8).into()),
        )
        .spacing(8);
        let heading_with_value = |heading: String, value: String| {
            padded_control(
                row![
                    text::heading(heading),
                    space::horizontal().width(Length::Fill),
                    text::body(value),
                ]
                .align_y(Alignment::Center),
            )
        };

        let display = column![padded_control(
            row![
                text::heading(fl!("show-dots")),
                space::horizontal().width(Length::Fill),
                toggler(config.show_dots).on_toggle(move |show_dots| Message::SetConfig(
                    WorkspacesConfig {
                        show_dots,
                        ..config
                    }
                )),
            ]
            .align_y(Alignment::Center)
        )]
        .push_maybe((!config.show_dots).then(|| {
            padded_control(
                row![
                    text::heading(fl!("show-circles")),
                    space::horizontal().width(Length::Fill),
                    toggler(config.show_circles).on_toggle(move |show_circles| {
                        Message::SetConfig(WorkspacesConfig {
                            show_circles,
                            ..config
                        })
                    }),
                ]
                .align_y(Alignment::Center),
            )
        }))
        .push_maybe((config.show_circles && !config.show_dots).then(|| {
            column![
                heading_with_value(fl!("circle-size"), format!("{} px", self.circle_size())),
                padded_control(slider(8..=32, self.circle_size(), move |size| {
                    Message::SetConfig(WorkspacesConfig {
                        circle_size: Some(size),
                        ..config
                    })
                })),
            ]
        }));

        let content = column![
            display,
            padded_control(divider::horizontal::default()),
            padded_control(text::heading(fl!("active-color"))),
            padded_control(swatches),
            padded_control(divider::horizontal::default()),
            heading_with_value(
                fl!("inactive-opacity"),
                format!("{:.0}%", config.inactive_opacity * 100.0)
            ),
            padded_control(
                slider(
                    0.2..=1.0,
                    config.inactive_opacity,
                    move |inactive_opacity| {
                        Message::SetConfig(WorkspacesConfig {
                            inactive_opacity,
                            ..config
                        })
                    }
                )
                .step(0.05)
            ),
            padded_control(divider::horizontal::default()),
            heading_with_value(fl!("spacing"), format!("{} px", self.spacing())),
            padded_control(slider(0..=16, self.spacing(), move |spacing| {
                Message::SetConfig(WorkspacesConfig {
                    spacing: Some(spacing),
                    ..config
                })
            })),
        ]
        .padding([8, 0]);

        self.core.applet.popup_container(container(content)).into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::CloseRequested(id))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
