package main

import (
	"context"
	"crypto/sha256"
	"fmt"
	"log"
	"strconv"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
)

// User represents a user in the database with a username and password.
type User struct {
	Username string `bson:"username"`
	Password string `bson:"password"`
}

// initializeDatabase initializes the MongoDB connection, inserts test data, and returns
// the client along with a cleanup function to disconnect from the database.
func initializeDatabase(url string) (*mongo.Client, func()) {
	log.Println("Generating test data...")

	newUsers := []interface{}{}

	for i := 0; i <= 500; i++ {
		suffix := strconv.Itoa(i)

		password := ""
		for j := 0; j < 10; j++ {
			password += suffix
		}
		sum := sha256.Sum256([]byte(password))

		user := User{
			Username: fmt.Sprintf("Cornell_%s", suffix),
			Password: fmt.Sprintf("%x", sum),
		}
		newUsers = append(newUsers, user)

		// Print the user and their hashed password
		fmt.Printf("Username: %s, Password: %s\n", user.Username, user.Password)
	}

	uri := fmt.Sprintf("mongodb://%s", url)
	log.Printf("Attempting connection to %v\n", uri)

	opts := options.Client().ApplyURI(uri)
	client, err := mongo.Connect(context.TODO(), opts)
	if err != nil {
		log.Panicf("Failed to connect to MongoDB: %v", err)
	}
	log.Println("Successfully connected to MongoDB")

	collection := client.Database("user-db").Collection("user")
	_, err = collection.InsertMany(context.TODO(), newUsers)
	if err != nil {
		log.Fatalf("Failed to insert test data into user DB: %v", err)
	}
	log.Println("Successfully inserted test data into user DB")

	// Return the client and a cleanup function to disconnect from the database.
	return client, func() {
		if err := client.Disconnect(context.TODO()); err != nil {
			log.Fatalf("Failed to disconnect from MongoDB: %v", err)
		}
		log.Println("Successfully disconnected from MongoDB")
	}
}

// queryUser queries and prints the user with the specified username from the database.
func queryUser(client *mongo.Client, username string) {
	collection := client.Database("user-db").Collection("user")
	var user User
	err := collection.FindOne(context.TODO(), bson.M{"username": username}).Decode(&user)
	if err != nil {
		log.Fatalf("Failed to find user: %v", err)
	}
	fmt.Printf("Found user: %+v\n", user)
}
