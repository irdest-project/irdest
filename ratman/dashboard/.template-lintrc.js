// SPDX-FileCopyrightText: 2022 embr <git@liclac.eu>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

'use strict';

module.exports = {
  extends: 'recommended',
  rules: {
    // Prevent unlocalised string literals in templates, use {{t '...'}}.
    // Unlocalisable strings, like arrows, emojis, etc. can be allowlisted here.
    'no-bare-strings': [
      // Front page metrics (upload/download).
      '↑',
      '↓',
    ],
  },
};
