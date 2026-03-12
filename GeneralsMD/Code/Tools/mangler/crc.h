#ifndef __CRC_H__
#define __CRC_H__

void Build_Packet_CRC(unsigned char *buf, int len); // len includes 4-byte CRC at head
bool Passes_CRC_Check(unsigned char *buf, int len); // len includes 4-byte CRC at head
void Add_CRC(unsigned long *crc, unsigned long val);

#endif // __CRC_H__
