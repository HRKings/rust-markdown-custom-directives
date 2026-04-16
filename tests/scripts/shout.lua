mdx.register_directive("shout", function(inv)
    local word = inv.attributes.word or "HELLO"
    return { type = "text", value = string.upper(word) }
end)
