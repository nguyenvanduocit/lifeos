//! The menu bar: what the app is doing, a pause for the agents, what macOS still has to grant, the server, opening at
//! login, quitting. It reads the shared state once a second.

use std::cell::OnceCell;
use std::sync::Arc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSApplicationActivationPolicy, NSControlStateValueOff,
    NSControlStateValueOn, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSTextField, NSVariableStatusItemLength,
    NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSTimer, NSURL};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

use crate::shared::{Shared, Status};

const ACCESSIBILITY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
const SCREEN_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

struct Items {
    status_item: Retained<NSStatusItem>,
    status: Retained<NSMenuItem>,
    pause: Retained<NSMenuItem>,
    accessibility: Retained<NSMenuItem>,
    screen: Retained<NSMenuItem>,
    server: Retained<NSMenuItem>,
    login: Retained<NSMenuItem>,
}

pub struct Ivars {
    shared: Arc<Shared>,
    items: OnceCell<Items>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Menu;

    impl Menu {
        #[unsafe(method(tick:))]
        fn tick(&self, _sender: Option<&AnyObject>) {
            self.refresh();
        }

        #[unsafe(method(togglePause:))]
        fn toggle_pause(&self, _sender: Option<&AnyObject>) {
            let shared = &self.ivars().shared;
            let _ = shared.set_paused(!shared.config().paused);
            self.refresh();
        }

        #[unsafe(method(grantAccessibility:))]
        fn grant_accessibility(&self, _sender: Option<&AnyObject>) {
            let _ = lifeos_desktop::Permissions::check(true);
            open_url(ACCESSIBILITY_URL);
        }

        #[unsafe(method(grantScreen:))]
        fn grant_screen(&self, _sender: Option<&AnyObject>) {
            let _ = lifeos_desktop::Permissions::check(true);
            open_url(SCREEN_URL);
        }

        #[unsafe(method(setServer:))]
        fn set_server(&self, _sender: Option<&AnyObject>) {
            self.ask_server();
        }

        #[unsafe(method(toggleLogin:))]
        fn toggle_login(&self, _sender: Option<&AnyObject>) {
            // SAFETY: plain ServiceManagement calls on the main app's own service.
            unsafe {
                let service = SMAppService::mainAppService();
                let result = if service.status() == SMAppServiceStatus::Enabled {
                    service.unregisterAndReturnError()
                } else {
                    service.registerAndReturnError()
                };
                if let Err(e) = result {
                    eprintln!("lifeos-computer: open at login: {}", e.localizedDescription());
                }
            }
            self.refresh();
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            std::process::exit(0);
        }
    }
);

impl Menu {
    fn new(shared: Arc<Shared>, mtm: MainThreadMarker) -> Retained<Menu> {
        let this = Menu::alloc(mtm).set_ivars(Ivars { shared, items: OnceCell::new() });
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }

    fn item(&self, menu: &NSMenu, title: &str, action: Option<objc2::runtime::Sel>, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
        // SAFETY: a fresh item; its target is this object, which lives as long as the app.
        unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(""),
            );
            if action.is_some() {
                item.setTarget(Some(self));
            } else {
                item.setEnabled(false);
            }
            menu.addItem(&item);
            item
        }
    }

    fn build(&self, mtm: MainThreadMarker) {
        let bar = NSStatusBar::systemStatusBar();
        let status_item = bar.statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str("cursorarrow.rays"),
                Some(&NSString::from_str("LifeOS Computer")),
            );
            match image {
                Some(i) => button.setImage(Some(&i)),
                None => button.setTitle(&NSString::from_str("LifeOS")),
            }
        }
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let status = self.item(&menu, "Connecting…", None, mtm);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let pause = self.item(&menu, "Pause agents", Some(sel!(togglePause:)), mtm);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let accessibility = self.item(&menu, "Accessibility", Some(sel!(grantAccessibility:)), mtm);
        let screen = self.item(&menu, "Screen Recording", Some(sel!(grantScreen:)), mtm);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let server = self.item(&menu, "Server…", Some(sel!(setServer:)), mtm);
        let login = self.item(&menu, "Open at login", Some(sel!(toggleLogin:)), mtm);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.item(&menu, "Quit LifeOS Computer", Some(sel!(quit:)), mtm);
        status_item.setMenu(Some(&menu));
        let _ = self.ivars().items.set(Items { status_item, status, pause, accessibility, screen, server, login });
        self.refresh();
    }

    fn refresh(&self) {
        let Some(items) = self.ivars().items.get() else { return };
        let shared = &self.ivars().shared;
        let config = shared.config();
        let host = url::Url::parse(&config.server).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or(config.server.clone());
        let status = match shared.status() {
            Status::Connecting => format!("Connecting to {host}…"),
            Status::Connected if config.paused => format!("Paused · connected to {host}"),
            Status::Connected => format!("Agents can use this Mac · {host}"),
            Status::Failed(why) => format!("Not connected: {}", why.chars().take(90).collect::<String>()),
        };
        items.status.setTitle(&NSString::from_str(&status));
        items.pause.setTitle(&NSString::from_str(if config.paused { "Resume agents" } else { "Pause agents" }));
        let p = lifeos_desktop::Permissions::check(false);
        let line = |name: &str, ok: bool| if ok { format!("{name}: allowed") } else { format!("{name}: not allowed — Open Settings…") };
        items.accessibility.setTitle(&NSString::from_str(&line("Accessibility", p.accessibility)));
        items.screen.setTitle(&NSString::from_str(&line("Screen Recording", p.screen_recording)));
        items.server.setTitle(&NSString::from_str(&format!("Server: {host}…")));
        // SAFETY: a plain status query.
        let at_login = unsafe { SMAppService::mainAppService().status() } == SMAppServiceStatus::Enabled;
        items.login.setState(if at_login { NSControlStateValueOn } else { NSControlStateValueOff });
        if let Some(button) = items.status_item.button(MainThreadMarker::from(self)) {
            let dim = config.paused || shared.status() != Status::Connected || !p.accessibility;
            button.setAppearsDisabled(dim);
        }
    }

    fn ask_server(&self) {
        let mtm = MainThreadMarker::from(self);
        let shared = &self.ivars().shared;
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("LifeOS server"));
        alert.setInformativeText(&NSString::from_str(
            "The address you open LifeOS at. This Mac must reach it (on the same tailnet).",
        ));
        alert.addButtonWithTitle(&NSString::from_str("Save"));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 24.0)));
        field.setStringValue(&NSString::from_str(&shared.config().server));
        alert.setAccessoryView(Some(&field));
        NSApplication::sharedApplication(mtm).activate();
        if alert.runModal() == NSAlertFirstButtonReturn {
            if let Err(e) = shared.set_server(&field.stringValue().to_string()) {
                let oops = NSAlert::new(mtm);
                oops.setMessageText(&NSString::from_str(&format!("{e:#}")));
                oops.runModal();
            }
        }
        self.refresh();
    }
}

fn open_url(url: &str) {
    if let Some(u) = NSURL::URLWithString(&NSString::from_str(url)) {
        NSWorkspace::sharedWorkspace().openURL(&u);
    }
}

/// Runs the menu bar until the app quits.
pub fn run(shared: Arc<Shared>) {
    let mtm = MainThreadMarker::new().expect("the menu bar runs on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let menu = Menu::new(shared, mtm);
    menu.build(mtm);
    // SAFETY: the timer's target lives as long as the app; tick: takes one object argument.
    unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(1.0, &menu, sel!(tick:), None, true);
    }
    app.run();
}
