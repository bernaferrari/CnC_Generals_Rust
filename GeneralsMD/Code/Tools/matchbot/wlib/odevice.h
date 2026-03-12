#ifndef ODEVICE_HEADER
#define ODEVICE_HEADER

// This virtual base class provides an interface for output devices
//  that can be used for the debugging package.
class OutputDevice
{
 public:
                 OutputDevice() {}
    virtual     ~OutputDevice() {};
    virtual int print(const char *s,int len)=0;
};

#endif
