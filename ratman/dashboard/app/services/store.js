// SPDX-FileCopyrightText: 2024 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

import BaseStore from 'ember-data/store';
import RequestManager from '@ember-data/request';
import Fetch from '@ember-data/request/fetch';
// import { CacheHandler } from '@ember-data/store';
import { LifetimesService } from '@ember-data/request-utils';

export default class Store extends BaseStore {
    constructor(args) {
        super(args);
        this.requestManager = new RequestManager();
        this.requestManager.use([ Fetch ]);
        // TODO: make the cache work :(
        // this.requestManager.useCache(CacheHandler);
        // this.lifetimes = new LifetimesService();
    }
}
