# Lowkey Workspaces

Fork of the COSMIC Numbered Workspaces applet. The active workspace just gets a coloured number instead of the accent circle. Right click it to change the colour, fade inactive workspaces, show dots instead of numbers or, adjust the spacing.

![Lowkey Workspaces](assets/preview.jpg)

Install (needs Rust and just):

    just install

Then add Lowkey Workspaces to your panel in COSMIC Settings.

Or as a Flatpak (needs flatpak-builder):

    flatpak-builder --user --install --force-clean build-dir dev.daenu.CosmicExtAppletLowkeyWorkspaces.json

Uninstall:

    just uninstall

GPL-3.0, based on https://github.com/pop-os/cosmic-applets
