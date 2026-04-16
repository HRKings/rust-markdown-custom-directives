mdx.register_directive("data_source", function(inv)
    return { type = "data", value = { count = 42, label = "items" } }
end)
