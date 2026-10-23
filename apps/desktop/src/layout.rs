/// Widths are logical pixels (points), shared by layout and visual acceptance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub sidebar: f32,
    pub compact: bool,
    pub split_inspector: bool,
    pub show_resources: bool,
    pub padding: f32,
    pub inspector: f32,
}
impl Layout {
    pub fn for_width(width: f32) -> Self {
        Self {
            sidebar: if width < 900. {
                64.
            } else if width < 1250. {
                184.
            } else {
                208.
            },
            compact: width < 900.,
            split_inspector: width >= 1280.,
            show_resources: width >= 1120.,
            padding: if width < 1250. { 20. } else { 28. },
            inspector: if width >= 1550. { 340. } else { 308. },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_preserves_list_width_and_keeps_details_accessible() {
        for width in [760., 900., 1040., 1280., 1440., 1920.] {
            let l = Layout::for_width(width);
            let available = width
                - l.sidebar
                - 2. * l.padding
                - if l.split_inspector {
                    l.inspector + 20.
                } else {
                    0.
                };
            assert!(available >= 640., "{width}: {available}");
            assert_eq!(l.split_inspector, width >= 1280.);
        }
    }
}
