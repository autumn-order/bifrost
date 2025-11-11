use std::collections::HashSet;

use chrono::{NaiveDateTime, Utc};
use dioxus::prelude::*;

use crate::model::user::CharacterDto;
use crate::model::user::{AllianceDto, CorporationDto};

fn format_relative_time(datetime: &NaiveDateTime) -> String {
    let now = Utc::now().naive_utc();
    let duration = now.signed_duration_since(*datetime);

    let seconds = duration.num_seconds();
    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if seconds < 60 {
        format!("{} seconds ago", seconds)
    } else if minutes < 60 {
        format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        )
    } else if hours < 24 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if days < 30 {
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else if days < 365 {
        let months = days / 30;
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    } else {
        let years = days / 365;
        format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
    }
}

#[component]
pub fn DashboardUpdateCard(characters: Signal<Vec<CharacterDto>>) -> Element {
    let characters_guard = characters.read();

    let mut alliance_ids = HashSet::new();
    let alliances: Vec<AllianceDto> = characters_guard
        .iter()
        .filter_map(|c| c.alliance.clone())
        .filter(|alliance| alliance_ids.insert(alliance.id))
        .collect();

    let mut corporation_ids = HashSet::new();
    let corporations: Vec<CorporationDto> = characters_guard
        .iter()
        .map(|c| c.corporation.clone())
        .filter(|corporation| corporation_ids.insert(corporation.id))
        .collect();

    let characters = characters_guard.clone();

    rsx!(
        div {
            class: "card shadow-sm w-full flex-1",
            div {
                class: "card-body",
                h2 {
                    class: "card-title",
                    "Update Information"
                }
                div {
                    class: "flex flex-col gap-4",
                    div {
                        class: "flex flex-col gap-2",
                        h2 { class: "text-lg", "Character Updates" }
                        CharacterTable { characters: characters }
                    }
                    div {
                        class: "flex flex-col gap-2",
                        h2 { class: "text-lg", "Corporation Updates" }
                        CorporationTable { corporations: corporations }
                    }
                    div {
                        class: "flex flex-col gap-2",
                        h2 { class: "text-lg", "Alliance Updates" }
                        AllianceTable { alliances: alliances }
                    }
                }
            }
        }
    )
}

#[component]
fn CharacterTable(characters: Vec<CharacterDto>) -> Element {
    rsx!(
        div {
            class: "overflow-x-auto",
            table {
                class: "table table-md",
                thead {
                    tr {
                        th { class: "w-48", "Character" }
                        th { class: "w-64", "Last Info Update (Monthly)" }
                        th { class: "w-64", "Last Affiliation Update (Hourly)" }
                    }
                }
                tbody {
                    {characters.iter().map(|character|
                        rsx!(
                            tr {
                                td { class: "w-48",
                                    "{character.name}"
                                }
                                td { class: "w-64",
                                    {format_relative_time(&character.info_updated_at)}
                                }
                                td { class: "w-64",
                                    {format_relative_time(&character.affiliation_updated_at)}
                                }
                            }
                        )
                    )}
                }
            }
        }
    )
}

#[component]
fn CorporationTable(corporations: Vec<CorporationDto>) -> Element {
    rsx!(
        div {
            class: "overflow-x-auto",
            table {
                class: "table table-md",
                thead {
                    tr {
                        th { class: "w-48", "Corporation" }
                        th { class: "w-64", "Last Info Update (Daily)" }
                        th { class: "w-64", "Last Affiliation Update (Hourly)" }
                    }
                }
                tbody {
                    {corporations.iter().map(|corporation|
                        rsx!(
                            tr {
                                td { class: "w-48",
                                    "{corporation.name}"
                                }
                                td { class: "w-64",
                                    {format_relative_time(&corporation.info_updated_at)}
                                }
                                td { class: "w-64",
                                    {format_relative_time(&corporation.affiliation_updated_at)}
                                }
                            }
                        )
                    )}
                }
            }
        }
    )
}

#[component]
fn AllianceTable(alliances: Vec<AllianceDto>) -> Element {
    rsx!(
        div {
            class: "overflow-x-auto",
            table {
                class: "table table-md",
                thead {
                    tr {
                        th { class: "w-48", "Alliance" }
                        th { class: "w-64", "Last Info Update (Daily)" }
                    }
                }
                tbody {
                    {alliances.iter().map(|alliance|
                        rsx!(
                            tr {
                                td { class: "w-48",
                                    "{alliance.name}"
                                }
                                td { class: "w-64",
                                    {format_relative_time(&alliance.updated_at)}
                                }
                            }
                        )
                    )}
                }
            }
        }
    )
}
