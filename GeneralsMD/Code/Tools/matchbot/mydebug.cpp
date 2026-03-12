#include <stdlib.h>
#include "mydebug.h"
#include "streamer.h"
#include "odevice.h"


// static MyMsgManager         *msg_manager=NULL;

// static int                paranoid_enabled=0;
static ostream           *paranoid_ostream=NULL;
static Streamer           paranoid_streamer;

// Don't dare touch this semaphore in application code!
#ifdef USE_SEM
Sem4                      MyDebugLibSemaphore;
#else
CritSec                      MyDebugLibSemaphore;
#endif


int MyMsgManager::setAllStreams(OutputDevice *device)
{
	if (device==NULL)
		return(1);

	MYDEBUGLOCK;
	paranoid_streamer.setOutputDevice(device);
	delete(paranoid_ostream);
	paranoid_ostream=new ostream(&paranoid_streamer);

	MYDEBUGUNLOCK;

	return(0);
}


int MyMsgManager::setParanoidStream(OutputDevice *device)
{
	if (device==NULL)
		return(1);

	MYDEBUGLOCK;
	paranoid_streamer.setOutputDevice(device);
	delete(paranoid_ostream);
	paranoid_ostream=new ostream(&paranoid_streamer);
	MYDEBUGUNLOCK;

	return(0);
}




ostream *MyMsgManager::paranoidStream(void)
{
	return(paranoid_ostream);
}


int MyMsgManager::ReplaceAllStreams(FileD * output_device, char *device_filename, char *copy_filename)
{
	MYDEBUGLOCK;

	delete(paranoid_ostream);

	if (output_device != NULL)
	{
		delete(output_device);
		output_device = NULL;
	}

	rename(device_filename, copy_filename);

	//      FileD new_device(device_filename);
	output_device = new FileD(device_filename);

	paranoid_streamer.setOutputDevice(output_device);
	paranoid_ostream = new ostream(&paranoid_streamer);

	MYDEBUGUNLOCK;

	return(0);
}
