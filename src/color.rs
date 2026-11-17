use crate::curses::Attr;
use crate::definitions::{Elem, NUMBER_OF_ELEMENTS};
use crate::global::Zu;

impl Zu {
    pub fn set_interface_colorpairs(&mut self) {
        let mut rescind_colors = std::env::var_os("NO_COLOR").is_some();
        let many_colors = self.colors > 15;
        for index in 0..NUMBER_OF_ELEMENTS {
            if let Some(combo) = self.color_combo[index] {
                self.interface_color_pair[index] = combo;
                rescind_colors = false;
            } else if index == Elem::FunctionTag as usize || index == Elem::ScrollBar as usize {
                self.interface_color_pair[index] = Attr::NORMAL;
            } else if index == Elem::GuideStripe as usize {
                self.interface_color_pair[index] = Attr::REVERSE;
            } else if index == Elem::Spotlighted as usize {
                self.interface_color_pair[index] =
                    Attr::colored(0, if many_colors { 11 } else { 3 }, 0);
            } else if index == Elem::MiniInfobar as usize || index == Elem::PromptBar as usize {
                self.interface_color_pair[index] =
                    self.interface_color_pair[Elem::TitleBar as usize];
            } else if index == Elem::ErrorMessage as usize {
                self.interface_color_pair[index] = Attr::colored(7, 1, crate::curses::A_BOLD);
            } else {
                self.interface_color_pair[index] = self.hilite_attribute;
            }
        }
        if rescind_colors {
            self.interface_color_pair[Elem::Spotlighted as usize] = Attr::REVERSE;
            self.interface_color_pair[Elem::ErrorMessage as usize] = Attr::REVERSE;
        }
    }
}
