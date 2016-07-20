Rust broad crawler
==================

Motivation
----------

Crawlers that need to crawl just one site (or a small number of them)
are usually not CPU bound, because they must be polite and avoid overloading
crawled sites with requests. On the other hand, broad crawls crawl a lot of
sites at the same time, and can crawl at a much higher rate, becoming CPU
bound.

This project is an experiment of making a simple broad crawler in Rust,
which should be less flexible than for example Scrapy crawlers, but faster.
A Python interface is possible, but not implemented at the moment.


Features
--------

This is a cralwer that downloads pages in breadth first order, following all
outbound links. It writes visited urls into ``urls.csv`` file (not really csv),
and page contents into ``out.jl`` file (definitely not a ``jl``).

It currently lacks politen scheduling, so please don't run it against a small
number of sites.


Running
-------

To start a broad crawl from seeds in ``top-1k.txt``, run::

    RUST_LOG=rust_broad_crawl=error cargo run --release top-1k.txt