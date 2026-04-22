# file-vec
"Uses memory mapping to store a vec like structure in the file system rather than on the heap. Although diskalloc already exists, I needed the option to make data persistent so an opaque allocator structure was inadequate for me. This reimplements most of Vec's features, with a couple extra stuff for relating to the backing file"
