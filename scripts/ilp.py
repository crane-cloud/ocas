import pulp

# Example data
microservices = [5, 8, 3, 6]  # Credit needs of each microservice
credits = [15, 10, 20]  # Available credits on each node

M = len(microservices)
N = len(credits)

# Create the problem
prob = pulp.LpProblem("MinimizeResourceImbalance", pulp.LpMinimize)

# Decision variables
x = pulp.LpVariable.dicts("x", ((i, j) for i in range(M) for j in range(N)), cat='Binary')

# Utilization of each node
u = [pulp.lpSum(microservices[i] * x[i, j] for i in range(M)) for j in range(N)]

# Average utilization
u_avg = pulp.lpSum(u) / N

# Objective function: Minimize the variance in utilization
prob += pulp.lpSum((u[j] - u_avg) ** 2 for j in range(N))

# Constraints
for i in range(M):
    prob += pulp.lpSum(x[i, j] for j in range(N)) == 1  # Each microservice assigned to one node

for j in range(N):
    prob += pulp.lpSum(microservices[i] * x[i, j] for i in range(M)) <= credits[j]  # Credit capacity constraint

# Solve the problem
prob.solve()

# Print the results
print(f"Status: {pulp.LpStatus[prob.status]}")
for i in range(M):
    for j in range(N):
        if pulp.value(x[i, j]) == 1:
            print(f"Microservice {i} assigned to Node {j}")
