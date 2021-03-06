Rust broad crawler
==================

Motivation
----------

Crawlers that need to crawl just one site (or a small number of them)
are usually not CPU bound, because they must be polite and avoid overloading
crawled sites with requests. On the other hand, broad crawlers
can crawl at a much higher rate, potentially becoming CPU bound.

This project is an experiment of making a simple broad crawler in Rust,
which should be less flexible than for example Scrapy crawlers, but hopefully
faster. A Python interface is possible, but is not implemented at the moment.


Features
--------

A cralwer downloads pages in breadth first order (using FIFO queue for each domain),
following all outbound links.
It writes all visited urls into ``urls.csv`` file,
and page contents into ``out.jl`` file in JSON-lines format.


Running
-------

To start a broad crawl from seeds in ``top-1k.txt``, run::

    RUST_LOG=crawler=info cargo run --release top-1k.txt

