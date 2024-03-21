FROM golang:1.19.4

# Install git
RUN apt-get update && apt-get install -y git

RUN git config --global http.sslverify false
COPY . /go/src/github.com/crane-cloud/ocas/hotelreservation
WORKDIR /go/src/github.com/crane-cloud/ocas/hotelreservation
# RUN go get gopkg.in/mgo.v2
# RUN go get github.com/bradfitz/gomemcache/memcache
# RUN go get github.com/google/uuid
# RUN go mod init
# RUN go mod vendor
RUN go install -ldflags="-s -w" ./cmd/...