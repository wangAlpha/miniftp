package main

import (
	"fmt"
	"net"
	"sync"
	"time"
	"os"
)

const (
	MAX_CONN = 2048
)

func main() {
	var wg sync.WaitGroup
	wg.Add(1)
	port := os.Args[1]
	// println("addr %s", addr)
	addr := fmt.Sprintf("127.0.0.1:%s", port)
	println(addr)
	for i := 0; i < MAX_CONN; i++ {
		go Conn(addr, i)
		time.Sleep(time.Millisecond * 10)
	}
	wg.Wait()
}

func Conn(addr string, id int) {
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		fmt.Println(err)
		return
	}
	fmt.Println("connect ", id)
	go func() {
		buf := make([]byte, 1024)
		for {
			n, err := conn.Read(buf)
			if err != nil {
				break
			}
			fmt.Println(id, "read: ", string(buf[:n]))
		}
	}()
	time.Sleep(time.Second * 1)
	for {
		_, err := conn.Write([]byte("hello"))
		if err != nil {
			break
		}
		time.Sleep(time.Second * 10)
	}
}
