// EA Pacific
// John McDonald, Jr
// Do not distribute

#pragma once

#ifndef _AUDIOREQUEST_H_
#define _AUDIOREQUEST_H_

#include "Common/GameAudio.h"
#include "Common/GameMemory.h"

class AudioEventRTS;

enum RequestType
{
	AR_Play,
	AR_Pause,
	AR_Stop
};

struct AudioRequest : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( AudioRequest, "AudioRequest" )

public:
	RequestType m_request;
	union 
	{
		AudioEventRTS *m_pendingEvent;
		AudioHandle m_handleToInteractOn;
	};
	Bool m_usePendingEvent;
	Bool m_requiresCheckForSample;
};

#endif // _AUDIOREQUEST_H_
