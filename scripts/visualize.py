from graphviz import Source as dot

path = "net.dot"
dot.from_file(path).render(view=True, format="png")
