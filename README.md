# MemDB
Memory everywhere. I want to remember everything. Everything is still a WIP

A hybrid database.

Document refers to markdown-esque files.

Implemented as an append-only log.

## Index
We index on Vectors, and tags.
We should also have FTS (full text search) via some inverted key-word index?
Index should live in memory, with a persistent disk backing in case of failures.
So in read path, just read from memory.
In write path, we do write-through--on document save, we update index in both the disk and memory

## Memory
We should also load as many LRU documents in $x$ allocated memory memory.
This should also be write-through.

### Search algorithm
-> query on index 
-> gets document ID(s)
    -> searches document ID -> memory KVP
        -> read
    -> if not found:
        -> searches document ID -> disk location in catalog
        -> read and return to user
        -> add to LRU

## Antirequisites
We probably shouldn't have a title.
- some documents may not have a title
- documents that do, we have to have an extract mechanism
- title has to be a good summary of the content
- would rather just have the tags summarize topics in the document

# File Persistance
We'll use log-structured hash-table (same architecture at Bitcask)
