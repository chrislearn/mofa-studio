//! MoFA FM Screen - Main screen for AI-powered audio streaming
//!
//! This module is split into sub-modules for better organization:
//! - `audio_controls.rs` - Audio device selection, mic monitoring
//! - `chat_panel.rs` - Chat display, prompt input
//! - `log_panel.rs` - Log display, filtering
//! - `dora_handlers.rs` - Dora event handling, dataflow control

mod audio_controls;
mod chat_panel;
mod log_panel;
mod dora_handlers;

use makepad_widgets::*;
use crate::mofa_hero::{MofaHeroWidgetExt, MofaHeroAction};
use crate::log_bridge;
use crate::dora_integration::{DoraIntegration, DoraCommand};
use mofa_widgets::participant_panel::ParticipantPanelWidgetExt;
use mofa_widgets::StateChangeListener;
use std::path::PathBuf;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use mofa_widgets::theme::*;
    use mofa_widgets::participant_panel::ParticipantPanel;
    use mofa_widgets::log_panel::LogPanel;
    use crate::mofa_hero::MofaHero;

    // Local layout constants (colors imported from theme)
    SECTION_SPACING = 12.0
    PANEL_RADIUS = 4.0
    PANEL_PADDING = 12.0

    // Reusable panel header style with dark mode support
    PanelHeader = <View> {
        width: Fill, height: Fit
        padding: {left: 16, right: 16, top: 12, bottom: 12}
        align: {y: 0.5}
        show_bg: true
        draw_bg: {
            instance dark_mode: 0.0
            fn pixel(self) -> vec4 {
                return mix((SLATE_50), (SLATE_800), self.dark_mode);
            }
        }
    }

    // Reusable vertical divider
    VerticalDivider = <View> {
        width: 1, height: Fill
        margin: {top: 4, bottom: 4}
        show_bg: true
        draw_bg: {
            instance dark_mode: 0.0
            fn pixel(self) -> vec4 {
                return mix((DIVIDER), (DIVIDER_DARK), self.dark_mode);
            }
        }
    }

    // Colang Screen - adaptive horizontal layout with left content and right log panel
    pub ColangScreen = {{ColangScreen}} {
        width: Fill, height: Fill
        flow: Right
        spacing: 0
        padding: { left: 16, right: 16, top: 16, bottom: 16 }
        align: {y: 0.0}
        show_bg: true
        draw_bg: {
            instance dark_mode: 0.0
            fn pixel(self) -> vec4 {
                return mix((DARK_BG), (DARK_BG_DARK), self.dark_mode);
            }
        }

        // Left column - main content area (adaptive width)
        left_column = <View> {
            width: Fill, height: Fill
            flow: Down
            spacing: (SECTION_SPACING)
            align: {y: 0.0}

            // System status bar (self-contained widget)
            mofa_hero = <MofaHero> {
                width: Fill
            }

            // Participant status cards container
            participant_container = <View> {
                width: Fill, height: Fit
                flow: Down
                spacing: 8

                participant_bar = <View> {
                    width: Fill, height: Fit
                    flow: Right
                    spacing: (SECTION_SPACING)

                    myself_panel = <ParticipantPanel> {
                        width: Fill, height: Fit
                        header = { name_label = { text: "Myself" } }
                    }
                    teacher_panel = <ParticipantPanel> {
                        width: Fill, height: Fit
                        header = { name_label = { text: "Teacher" } }
                    }
                }
            }

            // Chat window container (fills remaining space)
            chat_container = <View> {
                width: Fill, height: Fill
                flow: Down

                chat_section = <RoundedView> {
                    width: Fill, height: Fill
                    show_bg: true
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: (PANEL_RADIUS)
                        border_size: 1.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                            let bg = mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                            sdf.fill(bg);
                            sdf.stroke(border, self.border_size);
                            return sdf.result;
                        }
                    }
                    flow: Down

                    // Chat header with copy button
                    chat_header = <PanelHeader> {
                        chat_title = <Label> {
                            text: "Chat History"
                            draw_text: {
                                instance dark_mode: 0.0
                                text_style: <FONT_SEMIBOLD>{ font_size: 13.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((TEXT_PRIMARY), (TEXT_PRIMARY_DARK), self.dark_mode);
                                }
                            }
                        }
                        <Filler> {}
                        // Copy to clipboard button
                        copy_chat_btn = <Button> {
                            width: 28, height: 24
                            text: ""
                            draw_bg: {
                                instance hover: 0.0
                                instance pressed: 0.0
                                instance copied: 0.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    let c = self.rect_size * 0.5;

                                    // Background - flash green when copied
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 4.0);
                                    let bg_color = mix((BORDER), (GRAY_300), self.hover);
                                    let bg_color = mix(bg_color, (TEXT_MUTED), self.pressed);
                                    let bg_color = mix(bg_color, #22c55e, self.copied);
                                    sdf.fill(bg_color);

                                    // Icon color - white when copied for contrast
                                    let icon_color = mix((GRAY_600), #ffffff, self.copied);

                                    // Always draw clipboard, color changes to indicate success
                                    // Clipboard icon - back rectangle
                                    sdf.box(c.x - 4.0, c.y - 2.0, 8.0, 9.0, 1.0);
                                    sdf.stroke(icon_color, 1.2);

                                    // Clipboard icon - front rectangle (overlapping)
                                    sdf.box(c.x - 2.0, c.y - 5.0, 8.0, 9.0, 1.0);
                                    sdf.fill(bg_color);
                                    sdf.box(c.x - 2.0, c.y - 5.0, 8.0, 9.0, 1.0);
                                    sdf.stroke(icon_color, 1.2);

                                    return sdf.result;
                                }
                            }
                            animator: {
                                hover = {
                                    default: off
                                    off = { from: {all: Forward {duration: 0.1}} apply: {draw_bg: {hover: 0.0}} }
                                    on = { from: {all: Forward {duration: 0.1}} apply: {draw_bg: {hover: 1.0}} }
                                }
                                pressed = {
                                    default: off
                                    off = { from: {all: Forward {duration: 0.05}} apply: {draw_bg: {pressed: 0.0}} }
                                    on = { from: {all: Forward {duration: 0.02}} apply: {draw_bg: {pressed: 1.0}} }
                                }
                            }
                        }
                    }

                    // Chat messages area (scrollable, fills space)
                    chat_scroll = <ScrollYView> {
                        width: Fill, height: Fill
                        flow: Down
                        scroll_bars: <ScrollBars> {
                            show_scroll_x: false
                            show_scroll_y: true
                        }

                        chat_content_wrapper = <View> {
                            width: Fill, height: Fit
                            padding: (PANEL_PADDING)
                            flow: Down

                            chat_content = <Markdown> {
                                width: Fill, height: Fit
                                font_size: 13.0
                                font_color: (TEXT_PRIMARY)
                                paragraph_spacing: 8

                                draw_normal: {
                                    text_style: <FONT_REGULAR>{ font_size: 13.0 }
                                }
                                draw_bold: {
                                    text_style: <FONT_SEMIBOLD>{ font_size: 13.0 }
                                }
                            }
                        }
                    }
                }
            }

            // Audio control panel container - horizontal layout with individual containers
            audio_container = <View> {
                width: Fill, height: Fit
                flow: Right
                spacing: (SECTION_SPACING)

                // Mic level meter container
                mic_container = <RoundedView> {
                    width: Fit, height: Fit
                    padding: (PANEL_PADDING)
                    show_bg: true
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: (PANEL_RADIUS)
                        border_size: 1.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                            let bg = mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                            sdf.fill(bg);
                            sdf.stroke(border, self.border_size);
                            return sdf.result;
                        }
                    }

                    mic_group = <View> {
                        width: Fit, height: Fit
                        flow: Right
                        spacing: 10
                        align: {y: 0.5}

                        mic_mute_btn = <View> {
                            width: Fit, height: Fit
                            flow: Overlay
                            cursor: Hand
                            padding: 4

                            mic_icon_on = <View> {
                                width: Fit, height: Fit
                                <Icon> {
                                    draw_icon: {
                                        svg_file: dep("crate://self/resources/icons/mic.svg")
                                        fn get_color(self) -> vec4 { return (SLATE_500); }
                                    }
                                    icon_walk: {width: 20, height: 20}
                                }
                            }
                        }

                        mic_level_meter = <View> {
                            width: Fit, height: Fit
                            flow: Right
                            spacing: 3
                            align: {y: 0.5}
                            padding: {top: 2, bottom: 2}

                            mic_led_1 = <RoundedView> { width: 8, height: 14, draw_bg: { color: (GREEN_500), border_radius: 2.0 } }
                            mic_led_2 = <RoundedView> { width: 8, height: 14, draw_bg: { color: (GREEN_500), border_radius: 2.0 } }
                            mic_led_3 = <RoundedView> { width: 8, height: 14, draw_bg: { color: (SLATE_200), border_radius: 2.0 } }
                            mic_led_4 = <RoundedView> { width: 8, height: 14, draw_bg: { color: (SLATE_200), border_radius: 2.0 } }
                            mic_led_5 = <RoundedView> { width: 8, height: 14, draw_bg: { color: (SLATE_200), border_radius: 2.0 } }
                        }
                    }
                }

                // AEC toggle container
                aec_container = <RoundedView> {
                    width: Fit, height: Fit
                    padding: (PANEL_PADDING)
                    show_bg: true
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: (PANEL_RADIUS)
                        border_size: 1.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                            let bg = mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                            sdf.fill(bg);
                            sdf.stroke(border, self.border_size);
                            return sdf.result;
                        }
                    }

                    aec_group = <View> {
                        width: Fit, height: Fit
                        flow: Right
                        spacing: 8
                        align: {y: 0.5}

                        aec_toggle_btn = <View> {
                            width: Fit, height: Fit
                            padding: 6
                            flow: Overlay
                            cursor: Hand
                            show_bg: true
                            draw_bg: {
                                instance enabled: 1.0  // 1.0=on, 0.0=off
                                // Blink animation now driven by shader time - no timer needed!
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 4.0);
                                    let green = vec4(0.133, 0.773, 0.373, 1.0);
                                    let bright = vec4(0.2, 0.9, 0.5, 1.0);
                                    let gray = vec4(0.667, 0.686, 0.725, 1.0);
                                    // When enabled, pulse between green and bright green using shader time
                                    // sin(time * speed) creates smooth oscillation, step makes it blink
                                    let blink = step(0.0, sin(self.time * 2.0)) * self.enabled;
                                    let base = mix(gray, green, self.enabled);
                                    let col = mix(base, bright, blink * 0.5);
                                    sdf.fill(col);
                                    return sdf.result;
                                }
                            }
                            align: {x: 0.5, y: 0.5}

                            <Icon> {
                                draw_icon: {
                                    svg_file: dep("crate://self/resources/icons/aec.svg")
                                    fn get_color(self) -> vec4 { return (WHITE); }
                                }
                                icon_walk: {width: 20, height: 20}
                            }
                        }
                    }
                }

                // Device selectors container - fills remaining space
                device_container = <RoundedView> {
                    width: Fill, height: Fit
                    padding: (PANEL_PADDING)
                    show_bg: true
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: (PANEL_RADIUS)
                        border_size: 1.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                            let bg = mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                            sdf.fill(bg);
                            sdf.stroke(border, self.border_size);
                            return sdf.result;
                        }
                    }

                    device_selectors = <View> {
                        width: Fill, height: Fit
                        flow: Right
                        spacing: 16
                        align: {y: 0.5}

                        // Input device group (fills available space)
                        input_device_group = <View> {
                            width: Fill, height: Fit
                            flow: Right
                            spacing: 8
                            align: {y: 0.5}

                            input_device_label = <Label> {
                                width: 70  // Fixed width for alignment with output label
                                text: "Mic:"
                                draw_text: {
                                    instance dark_mode: 0.0
                                    text_style: <FONT_MEDIUM>{ font_size: 11.0 }
                                    fn get_color(self) -> vec4 {
                                        return mix((TEXT_SECONDARY), (TEXT_SECONDARY_DARK), self.dark_mode);
                                    }
                                }
                            }

                            input_device_dropdown = <DropDown> {
                                width: Fill, height: Fit
                                padding: {left: 10, right: 10, top: 6, bottom: 6}
                                popup_menu_position: BelowInput
                                // Labels will be set at runtime by init_audio()
                                labels: []
                                values: []
                                selected_item: 0
                                draw_bg: {
                                    instance dark_mode: 0.0
                                    fn pixel(self) -> vec4 {
                                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                        sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 3.0);
                                        let bg = mix((WHITE), (SLATE_700), self.dark_mode);
                                        sdf.fill(bg);
                                        return sdf.result;
                                    }
                                }
                                draw_text: {
                                    instance dark_mode: 0.0
                                    text_style: <FONT_REGULAR>{ font_size: 11.0 }
                                    fn get_color(self) -> vec4 {
                                        let light = mix((GRAY_700), (TEXT_PRIMARY), self.focus);
                                        let dark = mix((SLATE_300), (TEXT_PRIMARY_DARK), self.focus);
                                        return mix(light, dark, self.dark_mode);
                                    }
                                }
                                popup_menu: {
                                    width: 250  // Initial width - will be synced at runtime
                                    draw_bg: {
                                        instance dark_mode: 0.0
                                        border_size: 1.0
                                        fn pixel(self) -> vec4 {
                                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 2.0);
                                            let bg = mix((WHITE), (SLATE_800), self.dark_mode);
                                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                                            sdf.fill(bg);
                                            sdf.stroke(border, self.border_size);
                                            return sdf.result;
                                        }
                                    }
                                    menu_item: {
                                        width: Fill
                                        draw_bg: {
                                            instance dark_mode: 0.0
                                            fn pixel(self) -> vec4 {
                                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                                sdf.rect(0., 0., self.rect_size.x, self.rect_size.y);
                                                let base = mix((WHITE), (SLATE_800), self.dark_mode);
                                                let hover_color = mix((GRAY_100), (SLATE_700), self.dark_mode);
                                                sdf.fill(mix(base, hover_color, self.hover));
                                                return sdf.result;
                                            }
                                        }
                                        draw_text: {
                                            instance dark_mode: 0.0
                                            fn get_color(self) -> vec4 {
                                                let light_base = mix((GRAY_700), (TEXT_PRIMARY), self.active);
                                                let dark_base = mix((SLATE_300), (TEXT_PRIMARY_DARK), self.active);
                                                let base = mix(light_base, dark_base, self.dark_mode);
                                                let light_hover = (TEXT_PRIMARY);
                                                let dark_hover = (TEXT_PRIMARY_DARK);
                                                let hover_color = mix(light_hover, dark_hover, self.dark_mode);
                                                return mix(base, hover_color, self.hover);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        <VerticalDivider> {}

                        // Output device group (fills available space)
                        output_device_group = <View> {
                            width: Fill, height: Fit
                            flow: Right
                            spacing: 8
                            align: {y: 0.5}

                            output_device_label = <Label> {
                                width: 70  // Fixed width for alignment with input label
                                text: "Speaker:"
                                draw_text: {
                                    instance dark_mode: 0.0
                                    text_style: <FONT_MEDIUM>{ font_size: 11.0 }
                                    fn get_color(self) -> vec4 {
                                        return mix((TEXT_SECONDARY), (TEXT_SECONDARY_DARK), self.dark_mode);
                                    }
                                }
                            }

                            output_device_dropdown = <DropDown> {
                                width: Fill, height: Fit
                                padding: {left: 10, right: 10, top: 6, bottom: 6}
                                popup_menu_position: BelowInput
                                // Labels will be set at runtime by init_audio()
                                labels: []
                                values: []
                                selected_item: 0
                                draw_bg: {
                                    instance dark_mode: 0.0
                                    fn pixel(self) -> vec4 {
                                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                        sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 3.0);
                                        let bg = mix((WHITE), (SLATE_700), self.dark_mode);
                                        sdf.fill(bg);
                                        return sdf.result;
                                    }
                                }
                                draw_text: {
                                    instance dark_mode: 0.0
                                    text_style: <FONT_REGULAR>{ font_size: 11.0 }
                                    fn get_color(self) -> vec4 {
                                        let light = mix((GRAY_700), (TEXT_PRIMARY), self.focus);
                                        let dark = mix((SLATE_300), (TEXT_PRIMARY_DARK), self.focus);
                                        return mix(light, dark, self.dark_mode);
                                    }
                                }
                                popup_menu: {
                                    width: 250  // Initial width - will be synced at runtime
                                    draw_bg: {
                                        instance dark_mode: 0.0
                                        border_size: 1.0
                                        fn pixel(self) -> vec4 {
                                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 2.0);
                                            let bg = mix((WHITE), (SLATE_800), self.dark_mode);
                                            let border = mix((BORDER), (SLATE_600), self.dark_mode);
                                            sdf.fill(bg);
                                            sdf.stroke(border, self.border_size);
                                            return sdf.result;
                                        }
                                    }
                                    menu_item: {
                                        width: Fill
                                        draw_bg: {
                                            instance dark_mode: 0.0
                                            fn pixel(self) -> vec4 {
                                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                                sdf.rect(0., 0., self.rect_size.x, self.rect_size.y);
                                                let base = mix((WHITE), (SLATE_800), self.dark_mode);
                                                let hover_color = mix((GRAY_100), (SLATE_700), self.dark_mode);
                                                sdf.fill(mix(base, hover_color, self.hover));
                                                return sdf.result;
                                            }
                                        }
                                        draw_text: {
                                            instance dark_mode: 0.0
                                            fn get_color(self) -> vec4 {
                                                let light_base = mix((GRAY_700), (TEXT_PRIMARY), self.active);
                                                let dark_base = mix((SLATE_300), (TEXT_PRIMARY_DARK), self.active);
                                                let base = mix(light_base, dark_base, self.dark_mode);
                                                let light_hover = (TEXT_PRIMARY);
                                                let dark_hover = (TEXT_PRIMARY_DARK);
                                                let hover_color = mix(light_hover, dark_hover, self.dark_mode);
                                                return mix(base, hover_color, self.hover);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Prompt input area container
            prompt_container = <View> {
                width: Fill, height: Fit
                flow: Down

                prompt_section = <RoundedView> {
                    width: Fill, height: Fit
                    padding: (PANEL_PADDING)
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: (PANEL_RADIUS)
                        fn get_color(self) -> vec4 {
                            return mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                        }
                    }
                    flow: Down
                    spacing: 8

                    prompt_row = <View> {
                        width: Fill, height: Fit
                        flow: Right
                        spacing: 12
                        align: {y: 0.5}

                        prompt_input = <TextInput> {
                            width: Fill, height: Fit
                            padding: {left: 12, right: 12, top: 10, bottom: 10}
                            empty_text: "Enter message to send..."
                            draw_bg: {
                                instance dark_mode: 0.0
                                border_radius: 4.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                                    let bg = mix((SLATE_50), (SLATE_700), self.dark_mode);
                                    sdf.fill(bg);
                                    return sdf.result;
                                }
                            }
                            draw_text: {
                                instance dark_mode: 0.0
                                text_style: <FONT_REGULAR>{ font_size: 11.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((TEXT_PRIMARY), (TEXT_PRIMARY_DARK), self.dark_mode);
                                }
                            }
                            draw_selection: {
                                color: (INDIGO_200)
                            }
                        }

                        button_group = <View> {
                            width: Fit, height: Fit
                            flow: Right
                            spacing: 8

                            send_prompt_btn = <Button> {
                                width: Fit, height: 35
                                padding: {left: 16, right: 16}
                                text: "Send"
                                draw_text: {
                                    color: (WHITE)
                                    text_style: <FONT_SEMIBOLD>{ font_size: 11.0 }
                                }
                                draw_bg: {
                                    instance color: (ACCENT_BLUE)
                                    instance color_hover: (BLUE_700)
                                    border_radius: 4.0
                                    fn get_color(self) -> vec4 {
                                        return mix(self.color, self.color_hover, self.hover);
                                    }
                                    fn pixel(self) -> vec4 {
                                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                        sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                                        sdf.fill(self.get_color());
                                        return sdf.result;
                                    }
                                }
                            }

                            reset_btn = <Button> {
                                width: Fit, height: 35
                                padding: {left: 16, right: 16}
                                text: "Reset"
                                draw_text: {
                                    instance dark_mode: 0.0
                                    text_style: <FONT_MEDIUM>{ font_size: 11.0 }
                                    fn get_color(self) -> vec4 {
                                        return mix((GRAY_700), (SLATE_300), self.dark_mode);
                                    }
                                }
                                draw_bg: {
                                    instance dark_mode: 0.0
                                    border_radius: 4.0
                                    fn pixel(self) -> vec4 {
                                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                        sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                                        let base = mix((HOVER_BG), (SLATE_600), self.dark_mode);
                                        let hover_color = mix((SLATE_200), (SLATE_500), self.dark_mode);
                                        sdf.fill(mix(base, hover_color, self.hover));
                                        return sdf.result;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Splitter - draggable handle with padding
        splitter = <View> {
            width: 16, height: Fill
            margin: { left: 8, right: 8 }
            align: {y: 0.0}
            show_bg: true
            draw_bg: {
                instance dark_mode: 0.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    // Draw thin line in center
                    sdf.rect(7.0, 16.0, 2.0, self.rect_size.y - 32.0);
                    let color = mix((SLATE_300), (SLATE_600), self.dark_mode);
                    sdf.fill(color);
                    return sdf.result;
                }
            }
            cursor: ColResize
        }

        // System Log panel - adaptive width, top-aligned
        log_section = <View> {
            width: 320, height: Fill
            flow: Right
            align: {y: 0.0}

            // Toggle button column
            toggle_column = <View> {
                width: Fit, height: Fill
                show_bg: true
                draw_bg: {
                    instance dark_mode: 0.0
                    fn pixel(self) -> vec4 {
                        return mix((SLATE_50), (SLATE_800), self.dark_mode);
                    }
                }
                align: {x: 0.5, y: 0.0}
                padding: {left: 4, right: 4, top: 8}

                toggle_log_btn = <Button> {
                    width: Fit, height: Fit
                    padding: {left: 8, right: 8, top: 6, bottom: 6}
                    text: ">"
                    draw_text: {
                        instance dark_mode: 0.0
                        text_style: <FONT_BOLD>{ font_size: 11.0 }
                        fn get_color(self) -> vec4 {
                            return mix((SLATE_500), (SLATE_400), self.dark_mode);
                        }
                    }
                    draw_bg: {
                        instance dark_mode: 0.0
                        border_radius: 4.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                            let base = mix((SLATE_200), (SLATE_600), self.dark_mode);
                            let hover_color = mix((SLATE_300), (SLATE_500), self.dark_mode);
                            sdf.fill(mix(base, hover_color, self.hover));
                            return sdf.result;
                        }
                    }
                }
            }

            // Log content panel
            log_content_column = <RoundedView> {
                width: Fill, height: Fill
                draw_bg: {
                    instance dark_mode: 0.0
                    border_radius: (PANEL_RADIUS)
                    fn get_color(self) -> vec4 {
                        return mix((PANEL_BG), (PANEL_BG_DARK), self.dark_mode);
                    }
                }
                flow: Down

                log_header = <View> {
                    width: Fill, height: Fit
                    flow: Down
                    show_bg: true
                    draw_bg: {
                        instance dark_mode: 0.0
                        fn pixel(self) -> vec4 {
                            return mix((SLATE_50), (SLATE_800), self.dark_mode);
                        }
                    }

                    // Title row
                    log_title_row = <View> {
                        width: Fill, height: Fit
                        padding: {left: 12, right: 12, top: 10, bottom: 6}
                        log_title_label = <Label> {
                            text: "System Log"
                            draw_text: {
                                instance dark_mode: 0.0
                                text_style: <FONT_SEMIBOLD>{ font_size: 13.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((TEXT_PRIMARY), (TEXT_PRIMARY_DARK), self.dark_mode);
                                }
                            }
                        }
                    }

                    // Filter row
                    log_filter_row = <View> {
                        width: Fill, height: 32
                        flow: Right
                        align: {y: 0.5}
                        padding: {left: 8, right: 8, bottom: 6}
                        spacing: 6

                        // Level filter dropdown
                        level_filter = <DropDown> {
                            width: 70, height: 24
                            popup_menu_position: BelowInput
                            draw_bg: {
                                color: (HOVER_BG)
                                border_color: (SLATE_200)
                                border_radius: 2.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    // Background
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 2.0);
                                    sdf.fill((HOVER_BG));
                                    // Down arrow on right side
                                    let ax = self.rect_size.x - 12.0;
                                    let ay = self.rect_size.y * 0.5 - 2.0;
                                    sdf.move_to(ax - 3.0, ay);
                                    sdf.line_to(ax, ay + 4.0);
                                    sdf.line_to(ax + 3.0, ay);
                                    sdf.stroke((TEXT_PRIMARY), 1.5);
                                    return sdf.result;
                                }
                            }
                            draw_text: {
                                text_style: <FONT_MEDIUM>{ font_size: 10.0 }
                                fn get_color(self) -> vec4 {
                                    return (TEXT_PRIMARY);
                                }
                            }
                            popup_menu: {
                                draw_bg: {
                                    color: (WHITE)
                                    border_color: (BORDER)
                                    border_size: 1.0
                                    border_radius: 2.0
                                }
                                menu_item: {
                                    draw_bg: {
                                        color: (WHITE)
                                        color_hover: (GRAY_100)
                                    }
                                    draw_text: {
                                        fn get_color(self) -> vec4 {
                                            return mix(
                                                mix((GRAY_700), (TEXT_PRIMARY), self.active),
                                                (TEXT_PRIMARY),
                                                self.hover
                                            );
                                        }
                                    }
                                }
                            }
                            labels: ["ALL", "DEBUG", "INFO", "WARN", "ERROR"]
                            values: [ALL, DEBUG, INFO, WARN, ERROR]
                        }

                        // Node filter dropdown
                        node_filter = <DropDown> {
                            width: 85, height: 24
                            popup_menu_position: BelowInput
                            draw_bg: {
                                color: (HOVER_BG)
                                border_color: (SLATE_200)
                                border_radius: 2.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    // Background
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 2.0);
                                    sdf.fill((HOVER_BG));
                                    // Down arrow on right side
                                    let ax = self.rect_size.x - 12.0;
                                    let ay = self.rect_size.y * 0.5 - 2.0;
                                    sdf.move_to(ax - 3.0, ay);
                                    sdf.line_to(ax, ay + 4.0);
                                    sdf.line_to(ax + 3.0, ay);
                                    sdf.stroke((TEXT_PRIMARY), 1.5);
                                    return sdf.result;
                                }
                            }
                            draw_text: {
                                text_style: <FONT_MEDIUM>{ font_size: 10.0 }
                                fn get_color(self) -> vec4 {
                                    return (TEXT_PRIMARY);
                                }
                            }
                            popup_menu: {
                                draw_bg: {
                                    color: (WHITE)
                                    border_color: (BORDER)
                                    border_size: 1.0
                                    border_radius: 2.0
                                }
                                menu_item: {
                                    draw_bg: {
                                        color: (WHITE)
                                        color_hover: (GRAY_100)
                                    }
                                    draw_text: {
                                        fn get_color(self) -> vec4 {
                                            return mix(
                                                mix((GRAY_700), (TEXT_PRIMARY), self.active),
                                                (TEXT_PRIMARY),
                                                self.hover
                                            );
                                        }
                                    }
                                }
                            }
                            labels: ["All Nodes", "ASR", "TTS", "LLM", "Bridge", "Monitor", "App"]
                            values: [ALL, ASR, TTS, LLM, BRIDGE, MONITOR, APP]
                        }

                        // Search icon
                        search_icon = <View> {
                            width: 20, height: 24
                            align: {x: 0.5, y: 0.5}
                            show_bg: true
                            draw_bg: {
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    let c = self.rect_size * 0.5;
                                    // Magnifying glass circle
                                    sdf.circle(c.x - 2.0, c.y - 2.0, 5.0);
                                    sdf.stroke((GRAY_500), 1.5);
                                    // Handle
                                    sdf.move_to(c.x + 1.5, c.y + 1.5);
                                    sdf.line_to(c.x + 6.0, c.y + 6.0);
                                    sdf.stroke((GRAY_500), 1.5);
                                    return sdf.result;
                                }
                            }
                        }

                        // Search field
                        log_search = <TextInput> {
                            width: Fill, height: 24
                            empty_text: "Search..."
                            draw_bg: {
                                instance dark_mode: 0.0
                                border_radius: 2.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                                    let bg = mix((WHITE), (SLATE_700), self.dark_mode);
                                    sdf.fill(bg);
                                    return sdf.result;
                                }
                            }
                            draw_text: {
                                instance dark_mode: 0.0
                                text_style: <FONT_REGULAR>{ font_size: 10.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((TEXT_PRIMARY), (TEXT_PRIMARY_DARK), self.dark_mode);
                                }
                            }
                            draw_selection: {
                                color: (INDIGO_200)
                            }
                        }

                        // Copy to clipboard button
                        copy_log_btn = <Button> {
                            width: 28, height: 24
                            text: ""
                            draw_bg: {
                                instance hover: 0.0
                                instance pressed: 0.0
                                instance copied: 0.0
                                fn pixel(self) -> vec4 {
                                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                    let c = self.rect_size * 0.5;

                                    // Background - flash green when copied
                                    sdf.box(0., 0., self.rect_size.x, self.rect_size.y, 4.0);
                                    let bg_color = mix((BORDER), (GRAY_300), self.hover);
                                    let bg_color = mix(bg_color, (TEXT_MUTED), self.pressed);
                                    let bg_color = mix(bg_color, #22c55e, self.copied);
                                    sdf.fill(bg_color);

                                    // Icon color - white when copied for contrast
                                    let icon_color = mix((GRAY_600), #ffffff, self.copied);

                                    // Clipboard icon - back rectangle
                                    sdf.box(c.x - 4.0, c.y - 2.0, 8.0, 9.0, 1.0);
                                    sdf.stroke(icon_color, 1.2);

                                    // Clipboard icon - front rectangle (overlapping)
                                    sdf.box(c.x - 2.0, c.y - 5.0, 8.0, 9.0, 1.0);
                                    sdf.fill(bg_color);
                                    sdf.box(c.x - 2.0, c.y - 5.0, 8.0, 9.0, 1.0);
                                    sdf.stroke(icon_color, 1.2);

                                    return sdf.result;
                                }
                            }
                            animator: {
                                hover = {
                                    default: off
                                    off = { from: {all: Forward {duration: 0.1}} apply: {draw_bg: {hover: 0.0}} }
                                    on = { from: {all: Forward {duration: 0.1}} apply: {draw_bg: {hover: 1.0}} }
                                }
                                pressed = {
                                    default: off
                                    off = { from: {all: Forward {duration: 0.05}} apply: {draw_bg: {pressed: 0.0}} }
                                    on = { from: {all: Forward {duration: 0.02}} apply: {draw_bg: {pressed: 1.0}} }
                                }
                            }
                        }
                    }
                }

                log_scroll = <ScrollYView> {
                    width: Fill, height: Fill
                    flow: Down
                    scroll_bars: <ScrollBars> {
                        show_scroll_x: false
                        show_scroll_y: true
                    }

                    log_content_wrapper = <View> {
                        width: Fill, height: Fit
                        padding: { left: 12, right: 12, top: 8, bottom: 8 }
                        flow: Down

                        log_content = <Markdown> {
                            width: Fill, height: Fit
                            font_size: 10.0
                            font_color: (GRAY_600)
                            paragraph_spacing: 4

                            draw_normal: {
                                instance dark_mode: 0.0
                                text_style: <FONT_REGULAR>{ font_size: 10.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((GRAY_600), (SLATE_300), self.dark_mode);
                                }
                            }
                            draw_bold: {
                                instance dark_mode: 0.0
                                text_style: <FONT_SEMIBOLD>{ font_size: 10.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((GRAY_600), (SLATE_300), self.dark_mode);
                                }
                            }
                            draw_fixed: {
                                instance dark_mode: 0.0
                                text_style: <FONT_REGULAR>{ font_size: 9.0 }
                                fn get_color(self) -> vec4 {
                                    return mix((GRAY_600), (SLATE_300), self.dark_mode);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Chat message entry for display
#[derive(Clone, Debug)]
pub struct ChatMessageEntry {
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
    pub is_streaming: bool,
    pub session_id: Option<String>,
}

impl ChatMessageEntry {
    pub fn new(sender: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            sender: sender.into(),
            content: content.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            is_streaming: false,
            session_id: None,
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ColangScreen {
    #[deref]
    view: View,
    #[rust]
    log_panel_collapsed: bool,
    #[rust]
    log_panel_width: f64,
    #[rust]
    splitter_dragging: bool,
    #[rust]
    audio_manager: Option<crate::audio::AudioManager>,
    #[rust]
    audio_timer: Timer,
    #[rust]
    audio_initialized: bool,
    #[rust]
    input_devices: Vec<String>,
    #[rust]
    output_devices: Vec<String>,
    #[rust]
    log_level_filter: usize,  // 0=ALL, 1=DEBUG, 2=INFO, 3=WARN, 4=ERROR
    #[rust]
    log_node_filter: usize,   // 0=ALL, 1=ASR, 2=TTS, 3=LLM, 4=Bridge, 5=Monitor, 6=App
    #[rust]
    log_entries: Vec<String>,  // Raw log entries for filtering

    // Dropdown width caching for popup menu sync
    #[rust]
    dropdown_widths_initialized: bool,
    #[rust]
    cached_input_dropdown_width: f64,
    #[rust]
    cached_output_dropdown_width: f64,

    // AEC toggle state
    #[rust]
    aec_enabled: bool,
    // Note: AEC blink animation is now shader-driven (self.time), no timer needed

    // Dora integration
    #[rust]
    dora_integration: Option<DoraIntegration>,
    #[rust]
    dataflow_path: Option<PathBuf>,
    #[rust]
    dora_timer: Timer,
    #[rust]
    copy_chat_feedback_timer: Timer,
    #[rust]
    copy_log_feedback_timer: Timer,
    #[rust]
    chat_messages: Vec<ChatMessageEntry>,
    #[rust]
    last_chat_count: usize,
    // Pending streaming messages (updated in-place, removed when streaming ends)
    #[rust]
    pending_streaming_messages: Vec<ChatMessageEntry>,

    // Audio playback
    #[rust]
    audio_player: Option<std::sync::Arc<crate::audio_player::AudioPlayer>>,
    // Participant audio levels for decay animation (matches conference-dashboard)
    #[rust]
    participant_levels: [f64; 2],  // 0=myself, 1=teacher
}

impl Widget for ColangScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Initialize audio and log bridge on first event
        if !self.audio_initialized {
            // Initialize log bridge to capture Rust logs
            log_bridge::init();
            self.init_audio(cx);
            self.audio_initialized = true;
        }

        // Handle audio timer for mic level updates, log polling, and buffer status
        if self.audio_timer.is_event(event).is_some() {
            self.update_mic_level(cx);
            // Poll Rust logs (50ms interval is fine for log updates)
            self.poll_rust_logs(cx);
            // Send actual buffer fill percentage to dora for backpressure control
            // This replaces the bridge's estimation with the real value from AudioPlayer
            if let Some(ref player) = self.audio_player {
                let fill_percentage = player.buffer_fill_percentage();
                if let Some(ref dora) = self.dora_integration {
                    dora.send_command(DoraCommand::UpdateBufferStatus { fill_percentage });
                }
            }
            
            // Poll for captured audio chunks and send to dora
            if let Some(ref audio_manager) = self.audio_manager {
                let chunks = audio_manager.poll_audio_chunks();
                if !chunks.is_empty() {
                    if let Some(ref dora) = self.dora_integration {
                        for chunk in chunks {
                            ::log::debug!("Sending mic audio chunk: {} samples at {}Hz", 
                                chunk.samples.len(), chunk.sample_rate);
                            dora.send_audio(chunk.samples, chunk.sample_rate);
                        }
                    }
                }
            }
        }

        // Handle dora timer for polling dora events
        if self.dora_timer.is_event(event).is_some() {
            self.poll_dora_events(cx);
        }

        // Handle copy chat feedback timer - reset animation
        if self.copy_chat_feedback_timer.is_event(event).is_some() {
            self.view.button(ids!(left_column.chat_container.chat_section.chat_header.copy_chat_btn))
                .apply_over(cx, live!{ draw_bg: { copied: 0.0 } });
            self.view.redraw(cx);
        }

        // Handle copy log feedback timer - reset animation
        if self.copy_log_feedback_timer.is_event(event).is_some() {
            self.view.button(ids!(log_section.log_content_column.log_header.log_filter_row.copy_log_btn))
                .apply_over(cx, live!{ draw_bg: { copied: 0.0 } });
            self.view.redraw(cx);
        }

        // Handle AEC toggle button click
        // Note: AEC blink animation is now shader-driven, no timer needed
        let aec_btn = self.view.view(ids!(audio_container.aec_container.aec_group.aec_toggle_btn));
        match event.hits(cx, aec_btn.area()) {
            Hit::FingerUp(_) => {
                self.aec_enabled = !self.aec_enabled;
                let enabled_val = if self.aec_enabled { 1.0 } else { 0.0 };
                self.view.view(ids!(audio_container.aec_container.aec_group.aec_toggle_btn))
                    .apply_over(cx, live!{ draw_bg: { enabled: (enabled_val) } });
                self.view.redraw(cx);
            }
            _ => {}
        }

        // Handle splitter drag
        let splitter = self.view.view(ids!(splitter));
        match event.hits(cx, splitter.area()) {
            Hit::FingerDown(_) => {
                self.splitter_dragging = true;
            }
            Hit::FingerMove(fm) => {
                if self.splitter_dragging {
                    self.resize_log_panel(cx, fm.abs.x);
                }
            }
            Hit::FingerUp(_) => {
                self.splitter_dragging = false;
            }
            _ => {}
        }

        // Handle actions
        let actions = match event {
            Event::Actions(actions) => actions.as_slice(),
            _ => &[],
        };

        // Handle MofaHero start/stop actions
        for action in actions {
            match action.as_widget_action().cast() {
                MofaHeroAction::StartClicked => {
                    ::log::info!("Screen received StartClicked action");
                    self.handle_mofa_start(cx);
                }
                MofaHeroAction::StopClicked => {
                    ::log::info!("Screen received StopClicked action");
                    self.handle_mofa_stop(cx);
                }
                MofaHeroAction::None => {}
            }
        }

        // Handle toggle log panel button
        if self.view.button(ids!(log_section.toggle_column.toggle_log_btn)).clicked(actions) {
            self.toggle_log_panel(cx);
        }

        // Handle input device selection
        if let Some(item) = self.view.drop_down(ids!(audio_container.device_container.device_selectors.input_device_group.input_device_dropdown)).selected(actions) {
            if item < self.input_devices.len() {
                let device_name = self.input_devices[item].clone();
                self.select_input_device(cx, &device_name);
            }
        }

        // Handle output device selection
        if let Some(item) = self.view.drop_down(ids!(audio_container.device_container.device_selectors.output_device_group.output_device_dropdown)).selected(actions) {
            if item < self.output_devices.len() {
                let device_name = self.output_devices[item].clone();
                self.select_output_device(&device_name);
            }
        }

        // Handle log level filter dropdown
        if let Some(selected) = self.view.drop_down(ids!(log_section.log_content_column.log_header.log_filter_row.level_filter)).selected(actions) {
            self.log_level_filter = selected;
            self.update_log_display(cx);
        }

        // Handle log node filter dropdown
        if let Some(selected) = self.view.drop_down(ids!(log_section.log_content_column.log_header.log_filter_row.node_filter)).selected(actions) {
            self.log_node_filter = selected;
            self.update_log_display(cx);
        }

        // Handle copy log button
        if self.view.button(ids!(log_section.log_content_column.log_header.log_filter_row.copy_log_btn)).clicked(actions) {
            self.copy_logs_to_clipboard(cx);
            // Trigger copied feedback animation
            self.view.button(ids!(log_section.log_content_column.log_header.log_filter_row.copy_log_btn))
                .apply_over(cx, live!{ draw_bg: { copied: 1.0 } });
            self.view.redraw(cx);
            // Start timer to reset animation after 1 second
            self.copy_log_feedback_timer = cx.start_timeout(1.0);
        }

        // Handle copy chat button
        if self.view.button(ids!(left_column.chat_container.chat_section.chat_header.copy_chat_btn)).clicked(actions) {
            self.copy_chat_to_clipboard(cx);
            // Trigger copied feedback animation
            self.view.button(ids!(left_column.chat_container.chat_section.chat_header.copy_chat_btn))
                .apply_over(cx, live!{ draw_bg: { copied: 1.0 } });
            self.view.redraw(cx);
            // Start timer to reset animation after 1 second
            self.copy_chat_feedback_timer = cx.start_timeout(1.0);
        }

        // Handle log search text change
        if self.view.text_input(ids!(log_section.log_content_column.log_header.log_filter_row.log_search)).changed(actions).is_some() {
            self.update_log_display(cx);
        }

        // Handle Send button click
        if self.view.button(ids!(left_column.prompt_container.prompt_section.prompt_row.button_group.send_prompt_btn)).clicked(actions) {
            self.send_prompt(cx);
        }

        // Handle Reset button click
        if self.view.button(ids!(left_column.prompt_container.prompt_section.prompt_row.button_group.reset_btn)).clicked(actions) {
            self.reset_conversation(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Update popup menu widths to match dropdown widths
        // This handles first-frame zero width and caches values for performance
        let input_dropdown = self.view.drop_down(ids!(audio_container.device_container.device_selectors.input_device_group.input_device_dropdown));
        let input_width = input_dropdown.area().rect(cx).size.x;

        // Only update if width changed significantly (> 1px) to avoid unnecessary apply_over calls
        if input_width > 0.0 && (input_width - self.cached_input_dropdown_width).abs() > 1.0 {
            self.cached_input_dropdown_width = input_width;
            input_dropdown.apply_over(cx, live! {
                popup_menu: { width: (input_width) }
            });
        }

        let output_dropdown = self.view.drop_down(ids!(audio_container.device_container.device_selectors.output_device_group.output_device_dropdown));
        let output_width = output_dropdown.area().rect(cx).size.x;

        // Only update if width changed significantly (> 1px)
        if output_width > 0.0 && (output_width - self.cached_output_dropdown_width).abs() > 1.0 {
            self.cached_output_dropdown_width = output_width;
            output_dropdown.apply_over(cx, live! {
                popup_menu: { width: (output_width) }
            });
        }

        // Force an extra redraw on first frame to ensure widths are properly captured
        // This fixes the issue where first click shows narrow popup (width=0 on first frame)
        if !self.dropdown_widths_initialized {
            self.dropdown_widths_initialized = true;
            self.view.redraw(cx);
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl ColangScreenRef {
    /// Stop audio and dora timers - call this before hiding/removing the widget
    /// to prevent timer callbacks on inactive state
    /// Note: AEC blink animation is shader-driven and doesn't need stopping
    pub fn stop_timers(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow_mut() {
            cx.stop_timer(inner.audio_timer);
            cx.stop_timer(inner.dora_timer);
            ::log::debug!("ColangScreen timers stopped");
        }
    }

    /// Restart audio and dora timers - call this when the widget becomes visible again
    /// Note: AEC blink animation is shader-driven and auto-resumes
    pub fn start_timers(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.audio_timer = cx.start_interval(0.05);  // 50ms for mic level
            inner.dora_timer = cx.start_interval(0.1);    // 100ms for dora events
            ::log::debug!("ColangScreen timers started");
        }
    }
}

impl StateChangeListener for ColangScreenRef {
    fn on_dark_mode_change(&self, cx: &mut Cx, dark_mode: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            // Apply dark mode to screen background
            inner.view.apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to chat section
            inner.view.view(ids!(left_column.chat_container.chat_section)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to chat header and title
            inner.view.view(ids!(left_column.chat_container.chat_section.chat_header)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.label(ids!(left_column.chat_container.chat_section.chat_header.chat_title)).apply_over(cx, live!{
                draw_text: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to audio control containers
            inner.view.view(ids!(left_column.audio_container.mic_container)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.view(ids!(left_column.audio_container.aec_container)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.view(ids!(left_column.audio_container.device_container)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to device labels
            inner.view.label(ids!(left_column.audio_container.device_container.device_selectors.input_device_group.input_device_label)).apply_over(cx, live!{
                draw_text: { dark_mode: (dark_mode) }
            });
            inner.view.label(ids!(left_column.audio_container.device_container.device_selectors.output_device_group.output_device_label)).apply_over(cx, live!{
                draw_text: { dark_mode: (dark_mode) }
            });

            // NOTE: DropDown apply_over causes "target class not found" errors
            // TODO: Find alternative way to theme dropdowns

            // Apply dark mode to MofaHero
            inner.view.mofa_hero(ids!(left_column.mofa_hero)).update_dark_mode(cx, dark_mode);

            // Apply dark mode to participant panels
            inner.view.participant_panel(ids!(left_column.participant_container.participant_bar.myself_panel)).update_dark_mode(cx, dark_mode);
            inner.view.participant_panel(ids!(left_column.participant_container.participant_bar.teacher_panel)).update_dark_mode(cx, dark_mode);

            // Apply dark mode to prompt section
            inner.view.view(ids!(left_column.prompt_container.prompt_section)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            // NOTE: TextInput apply_over causes "target class not found" errors
            inner.view.button(ids!(left_column.prompt_container.prompt_section.prompt_row.button_group.reset_btn)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
                draw_text: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to splitter
            inner.view.view(ids!(splitter)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to log section - toggle column
            inner.view.view(ids!(log_section.toggle_column)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.button(ids!(log_section.toggle_column.toggle_log_btn)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
                draw_text: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to log section - log content column
            inner.view.view(ids!(log_section.log_content_column)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.view(ids!(log_section.log_content_column.log_header)).apply_over(cx, live!{
                draw_bg: { dark_mode: (dark_mode) }
            });
            inner.view.label(ids!(log_section.log_content_column.log_header.log_title_row.log_title_label)).apply_over(cx, live!{
                draw_text: { dark_mode: (dark_mode) }
            });

            // Apply dark mode to log content Markdown
            // Use apply_over with font_color - this works because font_color is a top-level property
            if dark_mode > 0.5 {
                inner.view.markdown(ids!(log_section.log_content_column.log_scroll.log_content_wrapper.log_content))
                    .apply_over(cx, live!{ font_color: (vec4(0.796, 0.835, 0.882, 1.0)) }); // SLATE_300
            } else {
                inner.view.markdown(ids!(log_section.log_content_column.log_scroll.log_content_wrapper.log_content))
                    .apply_over(cx, live!{ font_color: (vec4(0.294, 0.333, 0.388, 1.0)) }); // GRAY_600
            }

            inner.view.redraw(cx);
        }
    }
}
