////// NetCommandWrapperList.h ////////////////////////////////
// Bryan Cleveland

#pragma once

#ifndef __NETCOMMANDWRAPPERLIST_H
#define __NETCOMMANDWRAPPERLIST_H

#include "GameNetwork/NetCommandList.h"

class NetCommandWrapperListNode : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(NetCommandWrapperListNode, "NetCommandWrapperListNode")		
public:
	NetCommandWrapperListNode(NetWrapperCommandMsg *msg);
	//virtual ~NetCommandWrapperListNode();

	Bool isComplete();
	UnsignedShort getCommandID();
	UnsignedInt getRawDataLength();
	void copyChunkData(NetWrapperCommandMsg *msg);
	UnsignedByte * getRawData();

	Int getPercentComplete(void);

	NetCommandWrapperListNode *m_next;

protected:
	UnsignedShort m_commandID;
	UnsignedByte *m_data;
	UnsignedInt m_dataLength;
	Bool *m_chunksPresent;
	UnsignedInt m_numChunks;
	UnsignedInt m_numChunksPresent;

};

class NetCommandWrapperList : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(NetCommandWrapperList, "NetCommandWrapperList")		
public:
	NetCommandWrapperList();
	//virtual ~NetCommandWrapperList();

	void init();
	void reset();

	void processWrapper(NetCommandRef *ref);
	NetCommandList * getReadyCommands();

	Int getPercentComplete(UnsignedShort wrappedCommandID);

protected:
	void removeFromList(NetCommandWrapperListNode *node);

	NetCommandWrapperListNode *m_list;
};

#endif