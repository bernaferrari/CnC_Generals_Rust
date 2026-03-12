#pragma once

#ifndef __FRAMEDATA_H
#define __FRAMEDATA_H

#include "Lib/BaseType.h"
#include "GameNetwork/NetCommandList.h"

enum FrameDataReturnType {
	FRAMEDATA_NOTREADY,
	FRAMEDATA_RESEND,
	FRAMEDATA_READY
};

class FrameData {
public:
	FrameData();
	~FrameData();

	void init();
	void reset();
	void update();

	UnsignedInt getFrame();
	void setFrame(UnsignedInt frame);
	FrameDataReturnType allCommandsReady(Bool debugSpewage);
	NetCommandList * getCommandList();
	void setFrameCommandCount(UnsignedInt totalCommandCount);
	UnsignedInt getFrameCommandCount();
	void addCommand(NetCommandMsg *msg);
	UnsignedInt getCommandCount();
	void zeroFrame();
	void destroyGameMessages();

protected:
	UnsignedInt m_frame;
	UnsignedInt m_frameCommandCount;
	UnsignedInt m_commandCount;
	NetCommandList *m_commandList;
	UnsignedInt m_lastFailedCC;
	UnsignedInt m_lastFailedFrameCC;
};

#endif
