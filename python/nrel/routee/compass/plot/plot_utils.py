from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from matplotlib.colors import Colormap


class ColormapCircularIterator:
    def __init__(self, colormap: Colormap, num_colors: int):
        self.colormap = colormap
        self.num_colors = num_colors
        self.index = 0

    def __iter__(self) -> ColormapCircularIterator:
        return self

    def __next__(self) -> str:
        if not self.colormap:
            raise StopIteration
        normalized_value = self.index / float(self.num_colors)
        rgba = self.colormap(normalized_value)
        hex_color = rgba_to_hex(rgba)
        self.index = (self.index + 1) % self.num_colors
        return hex_color


RGBA_TUPLE = tuple[float, float, float, float]


def rgba_to_hex(rgba: RGBA_TUPLE) -> str:
    return f"#{int(rgba[0] * 255):02x}{int(rgba[1] * 255):02x}{int(rgba[2] * 255):02x}"
