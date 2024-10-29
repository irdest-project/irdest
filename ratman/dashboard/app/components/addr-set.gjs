function parseAddrs(addrs) {
  return Object.entries(addrs).map(([key, val]) => ({ key, val }));
}

<template>
  {{# each (parseAddrs @addrs) as | addr | }}
    <p>{{ addr.key }}</p>
  {{/ each }}
</template>
