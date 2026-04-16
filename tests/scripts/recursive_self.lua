mdx.register_directive("recursive_self", function(inv)
    return { type = "markdown", value = "before {{recursive_self}} after" }
end)
