// AppKit view rendering needs the process main thread, which libtest does not use.
#[cfg(target_os = "macos")]
mod harness {
    pub struct Agent {
        pub label: String,
        pub icon: Option<&'static str>,
    }

    pub fn agent(harness: Option<&str>) -> Agent {
        Agent {
            label: harness.unwrap_or("Claude").to_owned(),
            icon: harness.is_none().then_some("M8 1.5v13M1.5 8h13"),
        }
    }
}

#[cfg(target_os = "macos")]
mod usage {
    #[derive(Default)]
    pub struct UsageSnapshot {
        pub agents: Vec<AgentUsage>,
    }

    pub struct AgentUsage {
        pub harness: &'static str,
        pub limits: Vec<UsageLimit>,
    }

    pub struct UsageLimit {
        pub used_percent: f64,
    }
}

#[cfg(target_os = "macos")]
mod renderer {
    // Exercise the production implementation with minimal agent/snapshot fixtures.
    #![allow(dead_code)]
    include!("../src/menubar_usage.rs");

    pub fn check() {
        use crate::usage::{AgentUsage, UsageLimit};
        use objc2::{rc::autoreleasepool, MainThreadOnly};
        use objc2_app_kit::{NSAppearance, NSAppearanceCustomization, NSButton, NSColorSpace};

        autoreleasepool(|_| {
            let mtm = MainThreadMarker::new().expect("native test must run on the main thread");
            let snapshot = UsageSnapshot {
                agents: vec![
                    AgentUsage {
                        harness: "claude",
                        limits: vec![UsageLimit { used_percent: 12.0 }],
                    },
                    AgentUsage {
                        harness: "future agent",
                        limits: vec![UsageLimit { used_percent: 34.0 }],
                    },
                    AgentUsage {
                        harness: "api-only",
                        limits: vec![],
                    },
                ],
            };
            let title = attributed_readout(&snapshot).expect("quota readout");
            assert_eq!(
                title.string().to_string(),
                "\u{fffc} 12% · future agent 34%"
            );
            assert!(attributed_readout(&UsageSnapshot::default()).is_none());

            let button = NSButton::initWithFrame(
                NSButton::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 24.0)),
            );
            button.setBordered(false);
            let label = "BroGlow — Claude 12% · future agent 34%";
            set_button_readout(&button, Some(&snapshot), Some(label), label);
            let accessibility: Retained<NSString> =
                unsafe { msg_send![&*button, accessibilityLabel] };
            assert_eq!(accessibility.to_string(), label);
            button.setAttributedTitle(&title.attributedSubstringFromRange(NSRange::new(0, 1)));
            for (name, light_ink) in [
                ("NSAppearanceNameAqua", false),
                ("NSAppearanceNameDarkAqua", true),
                ("NSAppearanceNameAqua", false),
            ] {
                let appearance = NSAppearance::appearanceNamed(&NSString::from_str(name))
                    .expect("system appearance");
                button.setAppearance(Some(&appearance));
                let bitmap = button
                    .bitmapImageRepForCachingDisplayInRect(button.bounds())
                    .expect("offscreen bitmap");
                button.cacheDisplayInRect_toBitmapImageRep(button.bounds(), &bitmap);
                let mut sum = 0.0;
                let mut pixels = 0;
                for y in 0..bitmap.pixelsHigh() {
                    for x in 0..bitmap.pixelsWide() {
                        let color = bitmap
                            .colorAtX_y(x, y)
                            .unwrap()
                            .colorUsingColorSpace(&NSColorSpace::deviceRGBColorSpace())
                            .unwrap();
                        if color.alphaComponent() > 0.2 {
                            sum += (color.redComponent()
                                + color.greenComponent()
                                + color.blueComponent())
                                / 3.0;
                            pixels += 1;
                        }
                    }
                }
                assert!(pixels > 15, "agent glyph must have visible pixels");
                let brightness = sum / f64::from(pixels);
                assert!(
                    if light_ink {
                        brightness > 0.8
                    } else {
                        brightness < 0.2
                    },
                    "{name}: ink brightness {brightness}"
                );
            }
            set_button_readout(
                &button,
                Some(&UsageSnapshot::default()),
                Some("50k tok"),
                "BroGlow — 50k tok",
            );
            assert_eq!(button.attributedTitle().string().to_string(), "50k tok");
            set_button_readout(&button, Some(&snapshot), None, "BroGlow");
            assert_eq!(
                button.attributedTitle().length(),
                0,
                "hiding clears cached quotas"
            );
            set_button_readout(&button, Some(&snapshot), Some(label), label);
            set_button_readout(&button, None, None, "BroGlow");
            assert_eq!(
                button.attributedTitle().length(),
                0,
                "disabling clears native attachments"
            );
        });
        println!("Native menu bar usage checks passed (light → dark → light, labels, clearing).");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    renderer::check();
}
