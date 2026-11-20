//! Tamaño de la grilla en celdas y en píxeles.

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::grid::Dimensions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl GridSize {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cell_width: 8,
            cell_height: 16,
        }
    }

    pub fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.rows,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}
