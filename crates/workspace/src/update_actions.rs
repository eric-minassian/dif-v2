use ui::prelude::*;
use crate::ui_state::UpdateStatus;
use crate::updater;

use crate::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn spawn_update_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update_status = UpdateStatus::Checking;
        cx.notify();
        let view = cx.entity().clone();

        window
            .spawn(cx, async move |cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { updater::check_for_update() })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        match result {
                            Ok(Some(info)) => {
                                this.state.update_status = UpdateStatus::Available {
                                    version: info.version,
                                    download_url: info.download_url,
                                };
                            }
                            Ok(None) => {
                                this.state.update_status = UpdateStatus::Idle;
                            }
                            Err(_) => {
                                // Silently stay idle on check errors
                                this.state.update_status = UpdateStatus::Idle;
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_start_update(
        &mut self,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update_status = UpdateStatus::Updating;
        cx.notify();

        let view = cx.entity().clone();

        window
            .spawn(cx, async move |cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { updater::download_and_apply(&url) })
                    .await;

                // If we get here, the update failed (success exits the process)
                if let Err(e) = result {
                    cx.update(|_, cx| {
                        view.update(cx, |this, cx| {
                            this.state.update_status = UpdateStatus::Error(e);
                            cx.notify();
                        })
                    })
                    .ok();
                }
            })
            .detach();
    }
}
