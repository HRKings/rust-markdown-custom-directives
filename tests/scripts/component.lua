mdx.register_directive("alert", function(inv)
    local level = inv.attributes.level or "info"
    return { type = "component", name = "Alert", props = { level = level } }
end)
