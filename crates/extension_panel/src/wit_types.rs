use extension_host::wasm_host::wit;

// WIT type aliases for cleaner code
// These types come from the WIT definition in crates/extension_api/wit/since_v0.9.0/
pub type WitUiTree = wit::since_v0_9_0::ui_elements::UiTree;
pub type WitUiNode = wit::since_v0_9_0::ui_elements::UiNode;
pub type WitStyle = wit::since_v0_9_0::ui_elements::Style;
pub type WitColor = wit::since_v0_9_0::ui_elements::Color;
pub type WitLength = wit::since_v0_9_0::ui_elements::Length;
pub type WitDefiniteLength = wit::since_v0_9_0::ui_elements::DefiniteLength;
pub type WitAbsoluteLength = wit::since_v0_9_0::ui_elements::AbsoluteLength;
pub type WitBackground = wit::since_v0_9_0::ui_elements::Background;
pub type WitEdgesLength = wit::since_v0_9_0::ui_elements::EdgesLength;
pub type WitEdgesAbsolute = wit::since_v0_9_0::ui_elements::EdgesAbsolute;
pub type WitCornersAbsolute = wit::since_v0_9_0::ui_elements::CornersAbsolute;
pub type WitDivNode = wit::since_v0_9_0::ui_elements::DivNode;
pub type WitTextNode = wit::since_v0_9_0::ui_elements::TextNode;
pub type WitInputNode = wit::since_v0_9_0::ui_elements::InputNode;
pub type WitSvgNode = wit::since_v0_9_0::ui_elements::SvgNode;
pub type WitImgNode = wit::since_v0_9_0::ui_elements::ImgNode;
pub type WitIconSource = wit::since_v0_9_0::ui_elements::IconSource;
pub type WitUniformListNode = wit::since_v0_9_0::ui_elements::UniformListNode;
pub type WitUiEvent = wit::since_v0_9_0::gui::UiEvent;
pub type WitMouseEventData = wit::since_v0_9_0::gui::MouseEventData;
