// Copyright (C) 2020 Alexey Shvayka. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
esid: sec-object.issealed
description: >
  If Proxy "ownKeys" trap is missing, keys are sorted by type in ascending
  chronological order.
info: |
  TestIntegrityLevel ( O, level )

  [...]
  6. Let keys be ? O.[[OwnPropertyKeys]]().
  7. For each element k of keys, do
    a. Let currentDesc be ? O.[[GetOwnProperty]](k).

  [[OwnPropertyKeys]] ( )

  [...]
  6. If trap is undefined, then
    a. Return ? target.[[OwnPropertyKeys]]().

  OrdinaryOwnPropertyKeys ( O )

  [...]
  3. For each own property key P of O such that Type(P) is String and P is
  not an array index, in ascending chronological order of property creation, do
    a. Add P as the last element of keys.
  4. For each own property key P of O such that Type(P) is Symbol,
  in ascending chronological order of property creation, do
    a. Add P as the last element of keys.
  5. Return keys.
features: [Proxy, Symbol, Reflect]
includes: [compareArray.js]
---*/

var target = {};
var sym = Symbol();
target[sym] = 1;
target.foo = 2;
target[0] = 3;
Object.seal(target);

var getOwnKeys = [];
var proxy = new Proxy(target, {
  getOwnPropertyDescriptor: function(target, key) {
    getOwnKeys.push(key);
    return Reflect.getOwnPropertyDescriptor(target, key);
  },
});

Object.isSealed(proxy);
assert.compareArray(getOwnKeys, ["0", "foo", sym]);

reportCompare(0, 0);
