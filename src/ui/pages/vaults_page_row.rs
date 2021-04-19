// vaults_page_row.rs
//
// Copyright 2021 Martin Pobaschnig <mpobaschnig@posteo.de>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use adw::{subclass::prelude::*, PreferencesRowExt};
use glib::once_cell::sync::Lazy;
use glib::{clone, subclass};
use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use std::cell::RefCell;
use std::process::Command;

use super::{VaultsPageRowPasswordPromptDialog, VaultsPageRowSettingsDialog};
use crate::{backend::Backend, vault::*};

mod imp {

    use super::*;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/com/gitlab/mpobaschnig/Vaults/vaults_page_row.ui")]
    pub struct VaultsPageRow {
        #[template_child]
        pub vaults_page_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub open_folder_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub locker_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub settings_button: TemplateChild<gtk::Button>,

        pub config: RefCell<Option<VaultConfig>>,
        pub is_mounted: RefCell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VaultsPageRow {
        const NAME: &'static str = "VaultsPageRow";
        type ParentType = gtk::ListBoxRow;
        type Type = super::VaultsPageRow;

        fn new() -> Self {
            Self {
                vaults_page_row: TemplateChild::default(),
                open_folder_button: TemplateChild::default(),
                locker_button: TemplateChild::default(),
                settings_button: TemplateChild::default(),
                config: RefCell::new(None),
                is_mounted: RefCell::new(false),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VaultsPageRow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.setup_connect_handlers();

            self.open_folder_button.set_visible(false);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("remove", &[], glib::Type::UNIT.into()).build(),
                    Signal::builder("save", &[], glib::Type::UNIT.into()).build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for VaultsPageRow {}
    impl ListBoxRowImpl for VaultsPageRow {}
}

glib::wrapper! {
    pub struct VaultsPageRow(ObjectSubclass<imp::VaultsPageRow>)
        @extends gtk::Widget, gtk::ListBoxRow;
}

impl VaultsPageRow {
    pub fn connect_remove<F: Fn() + 'static>(&self, callback: F) -> glib::SignalHandlerId {
        self.connect_local("remove", false, move |_| {
            callback();
            None
        })
        .unwrap()
    }

    pub fn connect_save<F: Fn() + 'static>(&self, callback: F) -> glib::SignalHandlerId {
        self.connect_local("save", false, move |_| {
            callback();
            None
        })
        .unwrap()
    }

    pub fn new(vault: Vault) -> Self {
        let object: Self = glib::Object::new(&[]).expect("Failed to create VaultsPageRow");

        let self_ = &imp::VaultsPageRow::from_instance(&object);

        match (vault.get_name(), vault.get_config()) {
            (Some(name), Some(config)) => {
                self_.vaults_page_row.set_title(Some(&name));
                self_.config.replace(Some(config));
            }
            (_, _) => {
                log::error!("Vault(s) not initialised!");
            }
        }

        object
    }

    pub fn setup_connect_handlers(&self) {
        let self_ = imp::VaultsPageRow::from_instance(&self);

        self_
            .open_folder_button
            .connect_clicked(clone!(@weak self as obj => move |_| {
                obj.open_folder_button_clicked();
            }));

        self_
            .locker_button
            .connect_clicked(clone!(@weak self as obj => move |_| {
                obj.locker_button_clicked();
            }));

        self_
            .settings_button
            .connect_clicked(clone!(@weak self as obj => move |_| {
                obj.settings_button_clicked();
            }));
    }

    fn open_folder_button_clicked(&self) {
        let self_ = imp::VaultsPageRow::from_instance(&self);

        let output_res = Command::new("xdg-open")
            .arg(&self_.config.borrow().as_ref().unwrap().mount_directory)
            .output();

        if let Err(e) = output_res {
            log::error!("Failed to open folder: {}", e);
        }
    }

    fn locker_button_clicked(&self) {
        let self_ = imp::VaultsPageRow::from_instance(&self);
        let vault = self.get_vault();
        if *self_.is_mounted.borrow() {
            match Backend::close(&vault) {
                Ok(_) => {
                    *self_.is_mounted.borrow_mut() = false;
                    self_
                        .locker_button
                        .set_icon_name(&"changes-prevent-symbolic");
                    self_.open_folder_button.set_visible(false);
                    self_.settings_button.set_sensitive(true);
                }
                Err(e) => {
                    log::error!("Error closing vault: {}", e);
                }
            }
        } else {
            let dialog = VaultsPageRowPasswordPromptDialog::new();
            dialog.connect_response(clone!(@strong self as self2 => move |dialog, id|
                let password = dialog.get_password();
                dialog.destroy();
                match id {
                    gtk::ResponseType::Ok => {
                        match Backend::open(&vault, password) {
                            Ok(_) => {
                                let self2_ = imp::VaultsPageRow::from_instance(&self2);
                                *self2_.is_mounted.borrow_mut() = true;
                                self2_.locker_button.set_icon_name(&"changes-allow-symbolic");
                                self2_.open_folder_button.set_visible(true);
                                self2_.settings_button.set_sensitive(false);
                            }
                            Err(e) => {
                                log::error!("Error opening vault: {}", e);
                            }
                        }

                    }
                    _ => {}
                };
            ));

            dialog.show();
        }
    }

    fn settings_button_clicked(&self) {
        let dialog = VaultsPageRowSettingsDialog::new(self.get_vault());
        dialog.connect_response(clone!(@strong self as self2=> move |dialog, id|
            match id {
                gtk::ResponseType::Other(0) => {
                    self2.emit_by_name("remove", &[]).unwrap();

                    dialog.destroy();
                }
                gtk::ResponseType::Other(1) => {
                    self2.emit_by_name("save", &[]).unwrap();

                    dialog.destroy();
                }
                _ => {
                    dialog.destroy();
                }
        }));

        dialog.show();
    }

    pub fn get_vault(&self) -> Vault {
        let self_ = imp::VaultsPageRow::from_instance(&self);
        let name = self_.vaults_page_row.get_title();
        let config = self_.config.borrow().clone();
        match (name, config) {
            (Some(name), Some(config)) => Vault::new(
                name.to_string(),
                config.backend,
                config.encrypted_data_directory,
                config.mount_directory,
            ),
            (_, _) => Vault::new_none(),
        }
    }

    pub fn set_vault(&self, vault: Vault) {
        let self_ = imp::VaultsPageRow::from_instance(&self);
        let name = vault.get_name();
        let config = vault.get_config();
        match (name, config) {
            (Some(name), Some(config)) => {
                self_.vaults_page_row.set_title(Some(&name));
                self_.config.replace(Some(config));
            }
            (_, _) => {
                log::error!("Vault not initialised!");
            }
        }
    }

    pub fn get_name(&self) -> String {
        let self_ = imp::VaultsPageRow::from_instance(&self);
        self_.vaults_page_row.get_title().unwrap().to_string()
    }
}
