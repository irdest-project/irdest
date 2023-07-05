// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use netmod_inet::{InetEndpoint, InetError};

/// Parse a peer and introduce it to the appropriate netmod metadata
pub async fn attach_peers(ep: &InetEndpoint, p: Vec<&str>) -> Result<(), InetError> {
    let mut tcp = vec![];
    for peer in p {
        if peer == "" {
            continue;
        }

        let split: Vec<_> = peer.split('#').collect();
        let nmtt = match split.get(0) {
            Some(tt) => tt,
            None => {
                warn!("Missing driver identifier: {}", peer);
                continue;
            }
        };

        let rest = match split.get(1).map(Clone::clone) {
            Some(tt) => tt.to_string(),
            None => {
                warn!("Ignoring invalid peer info: {}", peer);
                continue;
            }
        };

        match nmtt {
            &"inet" => {
                debug!("Initialising '{}' peering session with: '{}'", nmtt, rest);
                tcp.push(rest)
            }
            tt => {
                warn!("Unknown peer type: {}", tt);
                continue;
            }
        }
    }

    ep.add_peers(tcp).await
}
