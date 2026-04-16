mdx.register_directive("fail", function(inv)
    return { type = "error", message = "intentional failure for testing" }
end)
