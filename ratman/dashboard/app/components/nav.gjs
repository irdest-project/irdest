import { LinkTo } from '@ember/routing';

<template>
  <ul>
    <li><LinkTo @route="index">Overview</LinkTo></li>
    <li><LinkTo @route="api">API</LinkTo></li>
  </ul>
</template>
