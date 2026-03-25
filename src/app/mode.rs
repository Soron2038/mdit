use std::ptr::NonNull;

use block2::StackBlock;
use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2::DefinedClass;
use objc2_app_kit::{NSAnimationContext, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::{CAMediaTimingFunction, kCAMediaTimingFunctionEaseInEaseOut};

use mdit::editor::view_mode::ViewMode;

use super::{AppDelegate, TAB_H, PATH_H, sidebar_target_frame, content_target_frame};

impl AppDelegate {
    /// Toggle between Viewer and Editor mode for the active tab.
    pub(super) fn toggle_mode(&self) {
        // 1. Collect state
        let (new_mode, text_view, editor_delegate, scroll_view) = {
            let tm = self.ivars().tab_manager.borrow();
            let tab = match tm.active() {
                Some(t) => t,
                None => return,
            };
            let new_mode = match tab.mode.get() {
                ViewMode::Viewer => ViewMode::Editor,
                ViewMode::Editor => ViewMode::Viewer,
            };
            tab.mode.set(new_mode);
            (
                new_mode,
                tab.text_view.clone(),
                tab.editor_delegate.clone(),
                tab.scroll_view.clone(),
            )
        };

        // 2. Non-visual changes (immediate)
        editor_delegate.set_mode(new_mode);
        text_view.setEditable(new_mode == ViewMode::Editor);
        if let Some(storage) = unsafe { text_view.textStorage() } {
            editor_delegate.reapply(&storage);
        }
        self.update_text_container_inset();

        // 3. Compute target frames
        let Some(win) = self.ivars().window.get() else { return };
        let bounds = win.contentView().unwrap().bounds();
        let (win_w, win_h) = (bounds.size.width, bounds.size.height);
        let content_h = (win_h - TAB_H - PATH_H).max(0.0);

        let target_sb_frame = sidebar_target_frame(new_mode, content_h);
        let find_offset = self.ivars().find.bar_height();
        let target_sv_frame = content_target_frame(new_mode, find_offset, win_w, win_h);

        // 4. Animated frame changes
        let Some(sb) = self.ivars().sidebar.get() else { return };

        let sb_ptr: *const NSView = sb.view();
        let sv_ptr: *const objc2_app_kit::NSScrollView = &*scroll_view;

        let animation_block = StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
            let ctx = unsafe { ctx.as_ref() };
            ctx.setDuration(0.35);
            let timing = CAMediaTimingFunction::functionWithName(unsafe { kCAMediaTimingFunctionEaseInEaseOut });
            ctx.setTimingFunction(Some(&*timing));

            let sb_proxy: *const AnyObject = unsafe { msg_send![sb_ptr, animator] };
            let _: () = unsafe { msg_send![sb_proxy, setFrame: target_sb_frame] };

            let sv_proxy: *const AnyObject = unsafe { msg_send![sv_ptr, animator] };
            let _: () = unsafe { msg_send![sv_proxy, setFrame: target_sv_frame] };
        });
        NSAnimationContext::runAnimationGroup_completionHandler(
            &animation_block,
            None::<&block2::DynBlock<dyn Fn()>>,
        );

        // Update find bar replace-row visibility based on new mode
        if self.ivars().find.is_open() {
            if let Some(fb) = self.ivars().find_bar.get() {
                let count = self.ivars().find.match_count();
                self.ivars().find.update_bar_height(fb, count, new_mode);
                // If bar height changed, update the find bar frame and scroll view
                let new_h = self.ivars().find.bar_height();
                let w = win.contentView().unwrap().bounds().size.width;
                fb.view().setFrame(NSRect::new(
                    NSPoint::new(0.0, PATH_H),
                    NSSize::new(w, new_h),
                ));
                let frame = self.content_frame();
                let tm = self.ivars().tab_manager.borrow();
                if let Some(t) = tm.active() {
                    t.scroll_view.setFrame(frame);
                }
            }
        }

        // Show/hide the word count field based on the new mode.
        if let Some(pb) = self.ivars().path_bar.get() {
            pb.set_wordcount_visible(new_mode == ViewMode::Editor, win_w);
            if new_mode == ViewMode::Editor {
                if let Some(storage) = unsafe { text_view.textStorage() } {
                    pb.update_wordcount(&storage.string().to_string());
                }
            }
        }
        self.update_welcome_visibility();
    }
}
