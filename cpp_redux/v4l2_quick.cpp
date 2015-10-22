#include <string>
#include <iostream>
#include <linux/videodev2.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <errno.h>
using namespace std;

static int xioctl(int fd, int request, void* arg) {
    int r;
    do {
        r = ioctl(fd, request, arg);
    } while (r == -1 && EINTR == errno);
    return r;
}

// class Camera {
// private:
//     int fd;
//
// public:
//     Camera();
// }
//
// Camera::Camera(&string path) {
//     fd = open(path);
// }

int main(int argc, char** argv) {
    char* path = "/dev/video0";
    uint8_t* buffer;

    int fd = open(path, O_RDWR);

    if (fd == -1) {
        // Couldn't find capture device
        return 1;
    }

    struct v4l2_capability caps = {0};
    if (xioctl(fd, VIDIOC_QUERYCAP, &caps) == -1) {
        // Camera doesn't support capture
        return 2;
    }

    struct v4l2_format fmt = {0};
    fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    fmt.fmt.pix.width = 320;
    fmt.fmt.pix.height = 240;
    fmt.fmt.pix.pixelformat = V4L2_PIX_FMT_MJPEG;
    fmt.fmt.pix.field = V4L2_FIELD_NONE;

    if (xioctl(fd, VIDIOC_S_FMT, &fmt) == -1) {
        // Error setting pixel format
        return 3;
    }

    struct v4l2_requestbuffers req = {0};
    req.count = 1;
    req.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    req.memory = V4L2_MEMORY_MMAP;

    if (xioctl(fd, VIDIOC_REQBUFS, &req) == -1) {
        // Error asking for mmap
        return 4;
    }

    struct v4l2_buffer buf = {0};
    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    buf.memory = V4L2_MEMORY_MMAP;
    buf.index = 0;

    buffer = mmap(NULL, buf.length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, buf.m.offset);

    if (xioctl(fd, VIDIOC_QBUF, &buf) == -1) {
        // Error querying buffer
        return 5;
    }

    if (xioctl(fd, VIDIOC_STREAMON, &buf.type) == -1) {
        // Error starting capture
        return 6;
    }


}
