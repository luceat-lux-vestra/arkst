from pathlib import Path

path = Path("crates/arkst-ir/src/lib.rs")
text = path.read_text()

needle = '''                caption_position: IrCaptionPositionInfo {
                    default: IrCaptionPosition::Top,
                    figures: Some(IrCaptionPosition::Bottom),
                    tables: None,
                    code_blocks: Some(IrCaptionPosition::Top),
                },
            }'''
replacement = '''                caption_position: IrCaptionPositionInfo {
                    default: IrCaptionPosition::Top,
                    figures: Some(IrCaptionPosition::Bottom),
                    tables: None,
                    code_blocks: Some(IrCaptionPosition::Top),
                },
                auto_page_break_max_depth: Some(2),
            }'''
if text.count(needle) != 1:
    raise SystemExit(f"first IR fixture anchor mismatch: {text.count(needle)}")
text = text.replace(needle, replacement, 1)

needle = '''            caption_position: IrCaptionPositionInfo {
                default: IrCaptionPosition::Top,
                figures: None,
                tables: Some(IrCaptionPosition::Bottom),
                code_blocks: None,
            },
        };'''
replacement = '''            caption_position: IrCaptionPositionInfo {
                default: IrCaptionPosition::Top,
                figures: None,
                tables: Some(IrCaptionPosition::Bottom),
                code_blocks: None,
            },
            auto_page_break_max_depth: None,
        };'''
if text.count(needle) != 1:
    raise SystemExit(f"second IR fixture anchor mismatch: {text.count(needle)}")
text = text.replace(needle, replacement, 1)

path.write_text(text)
