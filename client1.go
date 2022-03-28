package main

import (
	"fmt"
	"net"
	"sync"
	"time"
)

const (
	MAX_CONN = 10
)

func main() {
	var wg sync.WaitGroup
	wg.Add(1)
	for i := 0; i < MAX_CONN; i++ {
		go Conn("127.0.0.1:8090", i)
		time.Sleep(time.Millisecond * 100)
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
