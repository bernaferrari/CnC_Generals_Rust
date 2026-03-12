#pragma once

#ifndef __FRAMEDATAMANAGER_H
#define __FRAMEDATAMANAGER_H

#include "GameNetwork/NetworkDefs.h"
#include "GameNetwork/FrameData.h"

class FrameDataManager : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(FrameDataManager, "FrameDataManager")		
public:
	FrameDataManager(Bool isLocal);
	//virtual ~FrameDataManager();

	void init();
	void reset();
	void update();

	void addNetCommandMsg(NetCommandMsg *msg);
	void setIsLocal(Bool isLocal);
	FrameDataReturnType allCommandsReady(UnsignedInt frame, Bool debugSpewage);
	NetCommandList * getFrameCommandList(UnsignedInt frame);
	UnsignedInt getCommandCount(UnsignedInt frame);
	void setFrameCommandCount(UnsignedInt frame, UnsignedInt commandCount);
	UnsignedInt getFrameCommandCount(UnsignedInt frame);
	void zeroFrames(UnsignedInt startingFrame, UnsignedInt numFrames);
	void destroyGameMessages();
	void resetFrame(UnsignedInt frame, Bool isAdvancing = TRUE);
	void setQuitFrame(UnsignedInt frame);
	UnsignedInt getQuitFrame();
	Bool getIsQuitting();

protected:
	FrameData *m_frameData;
	Bool m_isLocal;

	Bool m_isQuitting;
	UnsignedInt m_quitFrame;
};

#endif
