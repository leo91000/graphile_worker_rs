mod actions;
mod add_job;
mod form;
mod job_details;

use leptos::prelude::*;

use self::actions::{FailJobsModal, RemoveKeyModal, RescheduleModal};
use self::add_job::AddJobModal;
use self::job_details::JobDetailsModal;
use super::api::RefreshSignals;
use super::filters::modal_title;
use super::types::{AdminClientConfig, Modal};

#[component]
pub(super) fn ModalView(
    modal: RwSignal<Option<Modal>>,
    config: AdminClientConfig,
    refresh: RefreshSignals,
) -> impl IntoView {
    view! {
        <div class="gw-modal" data-open=move || modal.get().is_some().to_string() role="dialog" aria-modal="true">
            <div class="gw-dialog">
                <div class="mb-4 flex items-center justify-between gap-3">
                    <h3 class="text-lg font-semibold">{move || modal_title(&modal.get())}</h3>
                    <button class="gw-btn" type="button" aria-label="Close" on:click=move |_| modal.set(None)>
                        <span class="i-lucide-x h-4 w-4"></span>
                    </button>
                </div>
                {move || match modal.get() {
                    Some(Modal::AddJob) => view! {
                        <AddJobModal
                            config=config.clone()
                            modal=modal
                            refresh=refresh
                        />
                    }.into_any(),
                    Some(Modal::FailJobs) => view! {
                        <FailJobsModal
                            config=config.clone()
                            modal=modal
                            refresh=refresh
                        />
                    }.into_any(),
                    Some(Modal::Reschedule) => view! {
                        <RescheduleModal
                            config=config.clone()
                            modal=modal
                            refresh=refresh
                        />
                    }.into_any(),
                    Some(Modal::RemoveKey) => view! {
                        <RemoveKeyModal
                            config=config.clone()
                            modal=modal
                            refresh=refresh
                        />
                    }.into_any(),
                    Some(Modal::JobDetails(job)) => view! { <JobDetailsModal job=job toast=refresh.toast /> }.into_any(),
                    None => ().into_any(),
                }}
            </div>
        </div>
    }
}
