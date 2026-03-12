// FILE: GameResultsThread.h //////////////////////////////////////////////////////
// Generals game results thread class interface
// Author: Matthew D. Campbell, August 2002

#pragma once

#ifndef __GAMERESULTSTHREAD_H__
#define __GAMERESULTSTHREAD_H__

#include "Common/SubsystemInterface.h"

// this class encapsulates a request for the thread
class GameResultsRequest
{
public:
	std::string hostname;
	UnsignedShort port;
	std::string results;
};

//-------------------------------------------------------------------------

// this class encapsulates a response from the thread
class GameResultsResponse
{
public:
	std::string hostname;
	UnsignedShort port;
	Bool sentOk;
};

//-------------------------------------------------------------------------

// this is the actual message queue used to pass messages between threads
class GameResultsInterface : public SubsystemInterface
{
public:
	virtual ~GameResultsInterface() {}
	virtual void startThreads( void ) = 0;
	virtual void endThreads( void ) = 0;
	virtual Bool areThreadsRunning( void ) = 0;

	virtual void addRequest( const GameResultsRequest& req ) = 0;
	virtual Bool getRequest( GameResultsRequest& resp ) = 0;

	virtual void addResponse( const GameResultsResponse& resp ) = 0;
	virtual Bool getResponse( GameResultsResponse& resp ) = 0;

	static GameResultsInterface* createNewGameResultsInterface( void );

	virtual Bool areGameResultsBeingSent( void ) = 0;
};

extern GameResultsInterface *TheGameResultsQueue;


#endif // __GAMERESULTSTHREAD_H__
