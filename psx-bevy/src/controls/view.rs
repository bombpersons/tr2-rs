//! Ordinary Bevy UI rendered into the PS1 canvas, before its screen pass.
//! Labels use the bitmap font in `psx::font`: vector text at this size breaks up.
use super::{BindingDevice, Button, ControlsMenu, ControlsOptions, DISCARD, Item, Mode, Page, RESTORE, SAVE};
use crate::{
    controller::ControllerBindings,
    pointer::{Hitboxes, node_rect},
    psx::{
        PsxView, SCREEN_PASS_ORDER,
        font::{GLYPH_W, LINE_H, text_image},
    },
};
use bevy::{camera::visibility::RenderLayers, prelude::*};

#[derive(Component)]
struct ControlsRoot;
#[derive(Component)]
struct ControlsCamera;
#[derive(Component)]
struct Row(usize);
#[derive(Component)]
struct Cell {
    row: usize,
    device: Option<BindingDevice>,
}
/// A binding column of one row: clicking it edits that device's slot.
#[derive(Component)]
struct Slot(Item);
#[derive(Component)]
struct Hint;
/// A header column: its text depends on the page.
#[derive(Component)]
struct Header(usize);
/// The page tabs in the title row, shown when the game registers options.
#[derive(Component)]
struct Tabs;
#[derive(Component)]
struct Tab(Page);
/// The text an `ImageNode` currently shows, so it is only re-rendered on change.
#[derive(Component)]
struct Label(String);

// These are framebuffer colours: this camera draws in the PSX encoded space.
const WHITE: Color = Color::linear_rgb(0.90, 0.94, 1.0);
const GOLD: Color = Color::linear_rgb(1.0, 0.85, 0.25);
const CYAN: Color = Color::linear_rgb(0.25, 0.85, 1.0);
const NAVY: Color = Color::linear_rgb(0.02, 0.04, 0.22);
const SELECTED: Color = Color::linear_rgb(0.06, 0.18, 0.48);

/// Inner width (320 minus border and padding) and column widths, in pixels.
const INNER: u32 = 304;
const ACTION_W: u32 = 70;
const BINDING_W: u32 = (INNER - ACTION_W) / 3;
/// Characters of a binding that fit its column after the `> ` marker.
const BINDING_CHARS: usize = (BINDING_W / GLYPH_W) as usize - 2;
/// Characters of an option's choice that fit between `< ` and ` >`.
const CHOICE_CHARS: usize = (BINDING_W / GLYPH_W) as usize - 4;
const HEADERS: [[&str; 4]; 2] = [["ACTION", "KEYBOARD", "GAMEPAD", "MOUSE"], ["SETTING", "VALUE", "", ""]];

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (refresh, collect_hitboxes));
}
fn label(text: &str, color: Color, images: &mut Assets<Image>) -> impl Bundle {
    let image = text_image(text);
    (Label(text.into()), size(&image), ImageNode::new(images.add(image)).with_color(color))
}
/// Exactly the image's size: one texel per canvas pixel, never stretched to the row.
fn size(image: &Image) -> Node {
    Node { width: px(image.width()), height: px(image.height()), flex_shrink: 0.0, ..default() }
}
/// A fixed-width column holding one label at its natural size.
fn column(width: u32) -> Node {
    Node { width: px(width), height: px(LINE_H), ..default() }
}
fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let camera = commands
        .spawn((
            Name::new("Controls camera"),
            Camera2d,
            Camera { order: SCREEN_PASS_ORDER - 1, is_active: false, clear_color: ClearColorConfig::None, ..default() },
            RenderLayers::layer(28),
            PsxView,
            ControlsCamera,
        ))
        .id();
    commands
        .spawn((
            Name::new("Controls menu"),
            ControlsRoot,
            UiTargetCamera(camera),
            Node {
                display: Display::None,
                width: percent(100),
                height: percent(100),
                padding: UiRect::axes(px(6), px(3)),
                border: UiRect::all(px(2)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(NAVY),
            BorderColor::all(CYAN),
        ))
        .with_children(|root| {
            root.spawn(Node {
                height: px(LINE_H + 3),
                flex_shrink: 0.0,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|title| {
                title.spawn(label("CONTROLLER SETUP", GOLD, &mut images));
                title
                    .spawn((Tabs, Node { display: Display::None, column_gap: px(GLYPH_W * 2), ..default() }))
                    .with_children(|tabs| {
                        tabs.spawn(label("TAB/R1", CYAN, &mut images));
                        tabs.spawn((Tab(Page::Bindings), label("BUTTONS", WHITE, &mut images)));
                        tabs.spawn((Tab(Page::Options), label("", WHITE, &mut images)));
                    });
            });
            root.spawn(Node { height: px(LINE_H), flex_shrink: 0.0, ..default() }).with_children(|header| {
                for (i, width) in [ACTION_W, BINDING_W, BINDING_W, BINDING_W].into_iter().enumerate() {
                    header.spawn(column(width)).with_child((Header(i), label(HEADERS[0][i], CYAN, &mut images)));
                }
            });
            for (row, button) in Button::ALL.into_iter().enumerate() {
                root.spawn((
                    Row(row),
                    Node { height: px(LINE_H), flex_shrink: 0.0, ..default() },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|line| {
                    line.spawn(column(ACTION_W))
                        .with_child((Cell { row, device: None }, label(button.label(), WHITE, &mut images)));
                    for device in BindingDevice::ALL {
                        line.spawn((column(BINDING_W), Slot(Item { row, device: Some(device) })))
                            .with_child((Cell { row, device: Some(device) }, label("", WHITE, &mut images)));
                    }
                });
            }
            for (row, text) in [(RESTORE, "Restore defaults"), (SAVE, "Save and close"), (DISCARD, "Discard and close")]
            {
                root.spawn((
                    Row(row),
                    Node { height: px(LINE_H), flex_shrink: 0.0, ..default() },
                    BackgroundColor(Color::NONE),
                ))
                .with_child((Cell { row, device: None }, label(text, WHITE, &mut images)));
            }
            // Statuses quote binding names of any length: clip rather than spill past the border.
            root.spawn(Node { margin: UiRect::top(px(2)), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() })
                .with_child((Hint, label("", CYAN, &mut images)));
        });
}

fn hint(mode: &Mode, device: BindingDevice, page: Page) -> &'static str {
    match mode {
        Mode::Browse if page == Page::Options => {
            "Up/down: select  Left/right/Enter: change\nDel/West: default  Esc/East: discard"
        }
        Mode::Listen { .. } if device == BindingDevice::Mouse => {
            "Click any mouse button; the next row follows.\nEsc: cancel this binding"
        }
        Mode::Browse => "Arrows/D-pad/mouse: select  Enter/click: edit\nDel/West: clear  Esc/East/right click: discard",
        Mode::Listen { .. } => {
            "Press one key or button; the next row follows.\nEsc or right click: cancel this binding"
        }
        Mode::Conflict(_) => "Enter/click: swap  Esc/East/right click: keep\nNothing changes until Save and close.",
    }
}

/// Rows, then the binding columns inside them, as screen rectangles.
fn collect_hitboxes(
    menu: Res<ControlsMenu>,
    options: Res<ControlsOptions>,
    rows: Query<(&Row, &ComputedNode, &UiGlobalTransform)>,
    slots: Query<(&Slot, &ComputedNode, &UiGlobalTransform)>,
    tabs: Query<(&Tab, &ComputedNode, &UiGlobalTransform)>,
    mut hitboxes: ResMut<Hitboxes<Item>>,
    mut tab_boxes: ResMut<Hitboxes<Page>>,
) {
    hitboxes.0.clear();
    tab_boxes.0.clear();
    if !menu.open {
        return;
    }
    // The options page hides the action rows it does not use, and its value
    // is the only column with a slot.
    let shown = |item: &Item| {
        menu.page == Page::Bindings
            || item.row >= RESTORE
            || (item.row < options.options.len() && item.device.is_none_or(|d| d == BindingDevice::Keyboard))
    };
    let rows = rows.iter().map(|(row, node, at)| (node_rect(node, at), Item { row: row.0, device: None }));
    let slots = slots.iter().map(|(slot, node, at)| (node_rect(node, at), slot.0));
    hitboxes.0.extend(rows.chain(slots).filter(|(_, item)| shown(item)));
    if !options.options.is_empty() {
        tab_boxes.0.extend(tabs.iter().map(|(tab, node, at)| (node_rect(node, at), tab.0)));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    menu: Res<ControlsMenu>,
    bindings: Res<ControllerBindings>,
    options: Res<ControlsOptions>,
    mut images: ResMut<Assets<Image>>,
    mut roots: Query<&mut Node, (With<ControlsRoot>, Without<Label>, Without<Row>, Without<Tabs>)>,
    mut cameras: Query<&mut Camera, With<ControlsCamera>>,
    mut rows: Query<(&Row, &mut BackgroundColor, &mut Node), Without<Label>>,
    mut cells: Query<(&Cell, &mut Label, &mut ImageNode, &mut Node), (Without<Hint>, Without<Header>, Without<Tab>)>,
    mut hints: Query<(&mut Label, &mut ImageNode, &mut Node), (With<Hint>, Without<Header>, Without<Tab>)>,
    mut headers: Query<(&Header, &mut Label, &mut ImageNode, &mut Node), (Without<Cell>, Without<Hint>, Without<Tab>)>,
    mut tabs: Query<&mut Node, (With<Tabs>, Without<Label>, Without<Row>)>,
    mut tab_labels: Query<
        (&Tab, &mut Label, &mut ImageNode, &mut Node),
        (Without<Cell>, Without<Hint>, Without<Header>),
    >,
) {
    if !menu.is_changed() && !bindings.is_changed() && !options.is_changed() {
        return;
    }
    for mut root in &mut roots {
        root.display = if menu.open { Display::Flex } else { Display::None };
    }
    for mut camera in &mut cameras {
        camera.is_active = menu.open;
    }
    if !menu.open {
        return;
    }
    let mut set = |label: &mut Label, node: &mut ImageNode, layout: &mut Node, text: String| {
        if label.0 != text {
            let image = text_image(&text);
            *layout = size(&image);
            node.image = images.add(image);
            label.0 = text;
        }
    };
    let draft = menu.draft.as_ref().unwrap_or(&bindings);
    let page = if options.options.is_empty() { Page::Bindings } else { menu.page };
    let choices = menu.options_draft.as_ref().unwrap_or(&options);
    for mut node in &mut tabs {
        node.display = if options.options.is_empty() { Display::None } else { Display::Flex };
    }
    for (tab, mut label, mut node, mut layout) in &mut tab_labels {
        node.color = if tab.0 == page { GOLD } else { WHITE };
        let text = match tab.0 {
            Page::Bindings => "BUTTONS".to_string(),
            Page::Options => options.title.clone(),
        };
        set(&mut label, &mut node, &mut layout, text);
    }
    for (header, mut label, mut node, mut layout) in &mut headers {
        let text = HEADERS[usize::from(page == Page::Options)][header.0];
        set(&mut label, &mut node, &mut layout, text.into());
    }
    for (row, mut color, mut layout) in &mut rows {
        color.0 = if row.0 == menu.selected { SELECTED } else { Color::NONE };
        let shown = page == Page::Bindings || row.0 >= RESTORE || row.0 < choices.options.len();
        layout.display = if shown { Display::Flex } else { Display::None };
    }
    for (cell, mut label, mut node, mut layout) in &mut cells {
        let selected = cell.row == menu.selected;
        if page == Page::Options && cell.row < RESTORE {
            let option = choices.options.get(cell.row);
            node.color = if selected { GOLD } else { WHITE };
            let text = match (cell.device, option) {
                (None, Some(option)) => option.label.clone(),
                (Some(BindingDevice::Keyboard), Some(option)) => {
                    let choice = option.choice().chars().take(CHOICE_CHARS).collect::<String>();
                    if selected { format!("< {choice} >") } else { choice }
                }
                _ => String::new(),
            };
            set(&mut label, &mut node, &mut layout, text);
            continue;
        }
        node.color = if selected && cell.device.is_none_or(|d| d == menu.device) { GOLD } else { WHITE };
        if let Some(device) = cell.device {
            let text = draft.binding(Button::ALL[cell.row], device).map_or("---".into(), |b| b.label());
            let text = text.chars().take(BINDING_CHARS).collect::<String>();
            let text = if selected && device == menu.device { format!("> {text}") } else { text };
            set(&mut label, &mut node, &mut layout, text);
        } else if cell.row < RESTORE {
            set(&mut label, &mut node, &mut layout, Button::ALL[cell.row].label().into());
        }
    }
    for (mut label, mut node, mut layout) in &mut hints {
        let text = format!("{}\n{}", menu.status, hint(&menu.mode, menu.device, page));
        set(&mut label, &mut node, &mut layout, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psx::font::text_width;

    #[test]
    fn layout_and_fixed_text_fit_the_canvas() {
        assert!(Button::ALL.iter().all(|b| text_width(b.label()) <= ACTION_W));
        for mode in [
            Mode::Browse,
            Mode::Listen { armed: false, pad: None },
            Mode::Conflict(crate::controller::BindingConflict {
                action: Button::Up,
                other: Button::Down,
                binding: crate::controller::PhysicalBinding::Key(KeyCode::KeyX),
            }),
        ] {
            for device in BindingDevice::ALL {
                for page in [Page::Bindings, Page::Options] {
                    assert!(text_width(hint(&mode, device, page)) <= INNER, "{}", hint(&mode, device, page));
                }
            }
        }
    }
}
