// SPDX-FileCopyrightText: 2022 embr <git@liclac.eu>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

import Route from '@ember/routing/route';
import { service } from '@ember/service';
import RSVP from 'rsvp';

export default class IndexRoute extends Route {
  @service store;
  // @service metrics;

  async model() {
    const res = await this.store.requestManager.request({
        url: "/api/v1/addrs"
    });

    return res.content;
  }
}
