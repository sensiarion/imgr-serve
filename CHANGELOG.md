Changelog
=========

0.1.4
--------

* fix bug with incorrect packing data, causes to reload image from api
* add limit to max version of the image storing into cache, it can be controlled via vars
    * `MAX_OPTIONS_PER_IMAGE_OVERFLOW_POLICY` (Rewrite or Restrict)
        * Restrict - will raise error when `MAX_OPTIONS_PER_IMAGE` is exceed
        * Rewrite - will drop last cache record to the new one

0.1.3
--------

* Add support for AVIF output extension (not as input)

0.1.2
--------

* Add env var to specify client cache ttl
* Preparation to full implementation of cache invalidation. Refactored key serialization for persistent storage. *
  *BREAKING:** Need to
  drop existing persistent storage before update (actually it's not breaking, but lead to dead weight in persistent
  cache)
* Add env var to restrict max output size(height, width)

0.1.1
--------

* Add calling persist for storage in background (for now it's every 60 secs. Planning to implement configurable
  persistency)

0.1.0
--------

* Initial version
