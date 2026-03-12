// FILE: PingThread.h //////////////////////////////////////////////////////
// Generals ping thread class interface
// Author: Matthew D. Campbell, August 2002
// Note: adapted from WOLAPI

#pragma once

#ifndef __PINGTHREAD_H__
#define __PINGTHREAD_H__

// this class encapsulates a request for the thread
class PingRequest
{
public:
	std::string hostname;
	Int repetitions;
	Int timeout;
};

//-------------------------------------------------------------------------

// this class encapsulates a response from the thread
class PingResponse
{
public:
	std::string hostname;
	Int avgPing;
	Int repetitions;
};

//-------------------------------------------------------------------------

// this is the actual message queue used to pass messages between threads
class PingerInterface
{
public:
	virtual ~PingerInterface() {}
	virtual void startThreads( void ) = 0;
	virtual void endThreads( void ) = 0;
	virtual Bool areThreadsRunning( void ) = 0;

	virtual void addRequest( const PingRequest& req ) = 0;
	virtual Bool getRequest( PingRequest& resp ) = 0;

	virtual void addResponse( const PingResponse& resp ) = 0;
	virtual Bool getResponse( PingResponse& resp ) = 0;

	static PingerInterface* createNewPingerInterface( void );

	virtual Bool arePingsInProgress( void ) = 0;
	virtual Int getPing( AsciiString hostname ) = 0;
	virtual void clearPingMap( void ) = 0;
	virtual AsciiString getPingString( Int timeout ) = 0;
};

extern PingerInterface *ThePinger;


#endif // __PINGTHREAD_H__
