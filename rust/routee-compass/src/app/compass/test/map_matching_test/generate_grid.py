import csv
import math

# Configuration
rows = 10
cols = 10
spacing = 0.01  # degrees, roughly 1km at equator

# Output files
nodes_file = "test_nodes.csv"
edges_file = "test_edges.csv"
geoms_file = "test_edge_geometries.txt"

# Data containers
nodes = []  # (id, x, y)
edges = []  # (id, src, dst, distance)
geoms = []  # "LINESTRING (x1 y1, x2 y2)"

# Generate Nodes
node_id_counter = 0
grid_ids = {}  # (row, col) -> id

# Base coordinates (Denver-ish area from existing test)
base_x = -105.0
base_y = 40.0

for r in range(rows):
    for c in range(cols):
        x = base_x + (c * spacing)
        y = base_y + (r * spacing)
        nodes.append((node_id_counter, x, y))
        grid_ids[(r, c)] = node_id_counter
        node_id_counter += 1

# Generate Edges (Grid connections: right and up)
edge_id_counter = 0


def dist(x1, y1, x2, y2):
    # Approximation for test data generation
    # 1 degree lat ~ 111km, 1 degree lon ~ 111km * cos(lat)
    # let's just use Euclidean distance in meters for simplicity in this generated set
    # assuming roughly meters
    dx = (x2 - x1) * 111000 * math.cos(math.radians(y1))
    dy = (y2 - y1) * 111000
    return math.sqrt(dx * dx + dy * dy)


for r in range(rows):
    for c in range(cols):
        src_id = grid_ids[(r, c)]
        src_x = nodes[src_id][1]
        src_y = nodes[src_id][2]

        # Connect Horizontal (Right)
        if c + 1 < cols:
            dst_id = grid_ids[(r, c + 1)]
            dst_x = nodes[dst_id][1]
            dst_y = nodes[dst_id][2]
            d = dist(src_x, src_y, dst_x, dst_y)

            # Forward edge
            edges.append((edge_id_counter, src_id, dst_id, d))
            geoms.append(f"LINESTRING ({src_x} {src_y}, {dst_x} {dst_y})")
            edge_id_counter += 1

            # Backward edge
            # edges.append((edge_id_counter, dst_id, src_id, d))
            # geoms.append(f"LINESTRING ({dst_x} {dst_y}, {src_x} {src_y})")
            # edge_id_counter += 1

        # Connect Vertical (Up)
        if r + 1 < rows:
            dst_id = grid_ids[(r + 1, c)]
            dst_x = nodes[dst_id][1]
            dst_y = nodes[dst_id][2]
            d = dist(src_x, src_y, dst_x, dst_y)

            # Forward edge
            edges.append((edge_id_counter, src_id, dst_id, d))
            geoms.append(f"LINESTRING ({src_x} {src_y}, {dst_x} {dst_y})")
            edge_id_counter += 1

            # Backward edge
            # edges.append((edge_id_counter, dst_id, src_id, d))
            # geoms.append(f"LINESTRING ({dst_x} {dst_y}, {src_x} {src_y})")
            # edge_id_counter += 1

# Write Nodes
with open(nodes_file, "w", newline="") as f:
    writer = csv.writer(f)
    writer.writerow(["vertex_id", "x", "y"])
    for n in nodes:
        writer.writerow(n)

# Write Edges
with open(edges_file, "w", newline="") as f:
    writer = csv.writer(f)
    writer.writerow(["edge_id", "src_vertex_id", "dst_vertex_id", "distance"])
    for e in edges:
        writer.writerow(e)

# Write Geometries
with open(geoms_file, "w") as f:
    for g in geoms:
        f.write(g + "\n")

print(f"Generated {len(nodes)} nodes and {len(edges)} edges.")
