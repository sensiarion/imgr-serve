Changelog
=========

0.1.2
--------

* Add env var to specify client cache ttl
* Preparation to full implementation of cache invalidation. Refactored key serialization for persistent storage. *
  *BREAKING:** Need to
  drop existing persistent storage before update (actually it's not breaking, but lead to dead weight in persistent
  cache).

0.1.1
--------

* Add calling persist for storage in background (for now it's every 60 secs. Planning to implement configurable
  persistency)

0.1.0
--------

* Initial version
