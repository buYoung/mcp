package main

import "fmt"

type UserService struct{}

func (service *UserService) Save() {}

func run() {
	service := &UserService{}
	service.Save()
	fmt.Println("ok")
}
