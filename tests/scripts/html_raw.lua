mdx.register_directive("html_raw", function(inv)
    local content = inv.attributes.content or "raw html"
    return { type = "html", value = '<div class="custom">' .. content .. '</div>' }
end)
