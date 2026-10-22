// Native, appearance-aware agent glyphs beside the status-light image.

use block2::RcBlock;
use objc2::{msg_send, rc::Retained, runtime::Bool, AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSAttributedStringAttachmentConveniences, NSButton, NSColor, NSCompositingOperation,
    NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext, NSImage,
    NSImageCacheMode, NSRectFillUsingOperation, NSTextAttachment,
};
use objc2_foundation::{
    NSAttributedString, NSData, NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize,
    NSString,
};

use crate::{harness, usage::UsageSnapshot};

/// Call on the tray event loop; a missing fallback removes the entire readout.
pub fn set_readout(
    tray: &tray_icon::TrayIcon,
    snapshot: Option<&UsageSnapshot>,
    fallback: Option<&str>,
) {
    tray.set_title(fallback);
    let tooltip = fallback.map_or_else(|| "BroGlow".to_owned(), |text| format!("BroGlow — {text}"));
    if let Some(button) = tray
        .ns_status_item()
        .and_then(|item| item.button(MainThreadMarker::new().expect("tray event loop")))
    {
        set_button_readout(&button, snapshot, fallback, &tooltip);
    }
    // tray-icon sizes its click overlay here, after AppKit lays out the attachments.
    let _ = tray.set_tooltip(Some(tooltip));
}

fn set_button_readout(
    button: &NSButton,
    snapshot: Option<&UsageSnapshot>,
    fallback: Option<&str>,
    label: &str,
) {
    button.setTitle(&NSString::from_str(fallback.unwrap_or("")));
    if let Some(title) = snapshot
        .filter(|_| fallback.is_some())
        .and_then(attributed_readout)
    {
        if let Some(font) = button.font() {
            unsafe {
                title.addAttribute_value_range(
                    NSFontAttributeName,
                    &font,
                    NSRange::new(0, title.length()),
                )
            };
        }
        button.setAttributedTitle(&title);
    }
    // Image attachments expose replacement characters without an explicit VoiceOver label.
    unsafe {
        let _: () = msg_send![button, setAccessibilityLabel: &*NSString::from_str(label)];
    }
}

fn attributed_readout(snapshot: &UsageSnapshot) -> Option<Retained<NSMutableAttributedString>> {
    let title = NSMutableAttributedString::initWithString(
        NSMutableAttributedString::alloc(),
        &NSString::from_str(""),
    );
    for usage in &snapshot.agents {
        let Some(limit) = usage.limits.first() else {
            continue;
        };
        if title.length() > 0 {
            append_text(&title, " · ");
        }
        let agent = harness::agent((usage.harness != "claude").then_some(usage.harness));
        if let Some(image) = agent.icon.and_then(agent_icon) {
            let attachment =
                NSTextAttachment::initWithData_ofType(NSTextAttachment::alloc(), None, None);
            attachment.setImage(Some(&image));
            attachment.setBounds(NSRect::new(
                NSPoint::new(0.0, -3.0),
                NSSize::new(14.0, 14.0),
            ));
            title.appendAttributedString(&NSAttributedString::attributedStringWithAttachment(
                &attachment,
            ));
        } else {
            append_text(&title, &agent.label);
        }
        append_text(&title, &format!(" {:.0}%", limit.used_percent));
    }
    if title.length() == 0 {
        return None;
    }
    unsafe {
        title.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &NSColor::labelColor(),
            NSRange::new(0, title.length()),
        );
    }
    Some(title)
}

fn append_text(title: &NSMutableAttributedString, text: &str) {
    title.appendAttributedString(&NSAttributedString::initWithString(
        NSAttributedString::alloc(),
        &NSString::from_str(text),
    ));
}

fn agent_icon(path: &str) -> Option<Retained<NSImage>> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="{path}" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
    );
    let source = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(svg.as_bytes()))?;
    let draw = RcBlock::new(move |rect| {
        NSGraphicsContext::saveGraphicsState_class();
        source.drawInRect(rect);
        NSColor::labelColor().setFill();
        NSRectFillUsingOperation(rect, NSCompositingOperation::SourceIn);
        NSGraphicsContext::restoreGraphicsState_class();
        Bool::YES
    });
    let image =
        NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(14.0, 14.0), false, &draw);
    // Template attachments stay black in dark mode; repaint with the current label color.
    image.setCacheMode(NSImageCacheMode::Never);
    Some(image)
}
