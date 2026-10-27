"""Finder layout; coordinates match assets/dmg-background.tiff (640 × 360)."""
import os

application = defines["app"]
format = "UDZO"
filesystem = "HFS+"
files = [application]
symlinks = {"Applications": "/Applications"}
# Do not set FinderInfo on the signed app (including the hide-extension flag):
# codesign --strict rejects that metadata on an application bundle.
background = defines["background"]
# Finder includes its 28-point title bar in the window bounds.
window_rect = ((200, 200), (640, 388))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
arrange_by = None
icon_size = 88
text_size = 13
icon_locations = {
    os.path.basename(application): (170, 180),
    "Applications": (470, 180),
    # Keep the artwork out of the layout even when Finder shows hidden files.
    ".background.tiff": (1000, 1000),
}
