/*
 * puff.c
 * Copyright (C) 2002-2013 Mark Adler
 * For conditions of distribution and use, see copyright notice in puff.h
 * version 2.3, 21 Jan 2013
 */
#include <setjmp.h>             
#include "puff.h"               
#define local static            
#define MAXBITS 15              
#define MAXLCODES 286           
#define MAXDCODES 30            
#define MAXCODES (MAXLCODES+MAXDCODES)  
#define FIXLCODES 288           
struct state {
    unsigned char *out;         
    unsigned long outlen;       
    unsigned long outcnt;       
    const unsigned char *in;    
    unsigned long inlen;        
    unsigned long incnt;        
    int bitbuf;                 
    int bitcnt;                 
    jmp_buf env;
};
local int bits(struct state *s, int need)
{
    long val;           
    val = s->bitbuf;
    while (s->bitcnt < need) {
        if (s->incnt == s->inlen)
            longjmp(s->env, 1);         
        val |= (long)(s->in[s->incnt++]) << s->bitcnt;  
        s->bitcnt += 8;
    }
    s->bitbuf = (int)(val >> need);
    s->bitcnt -= need;
    return (int)(val & ((1L << need) - 1));
}
local int stored(struct state *s)
{
    unsigned len;       
    s->bitbuf = 0;
    s->bitcnt = 0;
    if (s->incnt + 4 > s->inlen)
        return 2;                               
    len = s->in[s->incnt++];
    len |= s->in[s->incnt++] << 8;
    if (s->in[s->incnt++] != (~len & 0xff) ||
        s->in[s->incnt++] != ((~len >> 8) & 0xff))
        return -2;                              
    if (s->incnt + len > s->inlen)
        return 2;                               
    if (s->out != NIL) {
        if (s->outcnt + len > s->outlen)
            return 1;                           
        while (len--)
            s->out[s->outcnt++] = s->in[s->incnt++];
    }
    else {                                      
        s->outcnt += len;
        s->incnt += len;
    }
    return 0;
}
struct huffman {
    short *count;       
    short *symbol;      
};
local int decode(struct state *s, const struct huffman *h)
{
    int len;            
    int code;           
    int first;          
    int count;          
    int index;          
    int bitbuf;         
    int left;           
    short *next;        
    bitbuf = s->bitbuf;
    left = s->bitcnt;
    code = first = index = 0;
    len = 1;
    next = h->count + 1;
    while (1) {
        while (left--) {
            code |= bitbuf & 1;
            bitbuf >>= 1;
            count = *next++;
            if (code - count < first) { 
                s->bitbuf = bitbuf;
                s->bitcnt = (s->bitcnt - len) & 7;
                return h->symbol[index + (code - first)];
            }
            index += count;             
            first += count;
            first <<= 1;
            code <<= 1;
            len++;
        }
        left = (MAXBITS+1) - len;
        if (left == 0)
            break;
        if (s->incnt == s->inlen)
            longjmp(s->env, 1);         
        bitbuf = s->in[s->incnt++];
        if (left > 8)
            left = 8;
    }
    return -10;                         
}
local int construct(struct huffman *h, const short *length, int n)
{
    int symbol;         
    int len;            
    int left;           
    short offs[MAXBITS+1];      
    for (len = 0; len <= MAXBITS; len++)
        h->count[len] = 0;
    for (symbol = 0; symbol < n; symbol++)
        (h->count[length[symbol]])++;   
    if (h->count[0] == n)               
        return 0;                       
    left = 1;                           
    for (len = 1; len <= MAXBITS; len++) {
        left <<= 1;                     
        left -= h->count[len];          
        if (left < 0)
            return left;                
    }                                   
    offs[1] = 0;
    for (len = 1; len < MAXBITS; len++)
        offs[len + 1] = offs[len] + h->count[len];
    for (symbol = 0; symbol < n; symbol++)
        if (length[symbol] != 0)
            h->symbol[offs[length[symbol]]++] = symbol;
    return left;
}
local int codes(struct state *s,
                const struct huffman *lencode,
                const struct huffman *distcode)
{
    int symbol;         
    int len;            
    unsigned dist;      
    static const short lens[29] = { 
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
        35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
    static const short lext[29] = { 
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
        3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
    static const short dists[30] = { 
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
        257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
        8193, 12289, 16385, 24577};
    static const short dext[30] = { 
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
        7, 7, 8, 8, 9, 9, 10, 10, 11, 11,
        12, 12, 13, 13};
    do {
        symbol = decode(s, lencode);
        if (symbol < 0)
            return symbol;              
        if (symbol < 256) {             
            if (s->out != NIL) {
                if (s->outcnt == s->outlen)
                    return 1;
                s->out[s->outcnt] = symbol;
            }
            s->outcnt++;
        }
        else if (symbol > 256) {        
            symbol -= 257;
            if (symbol >= 29)
                return -10;             
            len = lens[symbol] + bits(s, lext[symbol]);
            symbol = decode(s, distcode);
            if (symbol < 0)
                return symbol;          
            dist = dists[symbol] + bits(s, dext[symbol]);
#ifndef INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR
            if (dist > s->outcnt)
                return -11;     
#endif
            if (s->out != NIL) {
                if (s->outcnt + len > s->outlen)
                    return 1;
                while (len--) {
                    s->out[s->outcnt] =
#ifdef INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR
                        dist > s->outcnt ?
                            0 :
#endif
                            s->out[s->outcnt - dist];
                    s->outcnt++;
                }
            }
            else
                s->outcnt += len;
        }
    } while (symbol != 256);            
    return 0;
}
local int fixed(struct state *s)
{
    static int virgin = 1;
    static short lencnt[MAXBITS+1], lensym[FIXLCODES];
    static short distcnt[MAXBITS+1], distsym[MAXDCODES];
    static struct huffman lencode, distcode;
    if (virgin) {
        int symbol;
        short lengths[FIXLCODES];
        lencode.count = lencnt;
        lencode.symbol = lensym;
        distcode.count = distcnt;
        distcode.symbol = distsym;
        for (symbol = 0; symbol < 144; symbol++)
            lengths[symbol] = 8;
        for (; symbol < 256; symbol++)
            lengths[symbol] = 9;
        for (; symbol < 280; symbol++)
            lengths[symbol] = 7;
        for (; symbol < FIXLCODES; symbol++)
            lengths[symbol] = 8;
        construct(&lencode, lengths, FIXLCODES);
        for (symbol = 0; symbol < MAXDCODES; symbol++)
            lengths[symbol] = 5;
        construct(&distcode, lengths, MAXDCODES);
        virgin = 0;
    }
    return codes(s, &lencode, &distcode);
}
local int dynamic(struct state *s)
{
    int nlen, ndist, ncode;             
    int index;                          
    int err;                            
    short lengths[MAXCODES];            
    short lencnt[MAXBITS+1], lensym[MAXLCODES];         
    short distcnt[MAXBITS+1], distsym[MAXDCODES];       
    struct huffman lencode, distcode;   
    static const short order[19] =      
        {16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15};
    lencode.count = lencnt;
    lencode.symbol = lensym;
    distcode.count = distcnt;
    distcode.symbol = distsym;
    nlen = bits(s, 5) + 257;
    ndist = bits(s, 5) + 1;
    ncode = bits(s, 4) + 4;
    if (nlen > MAXLCODES || ndist > MAXDCODES)
        return -3;                      
    for (index = 0; index < ncode; index++)
        lengths[order[index]] = bits(s, 3);
    for (; index < 19; index++)
        lengths[order[index]] = 0;
    err = construct(&lencode, lengths, 19);
    if (err != 0)               
        return -4;
    index = 0;
    while (index < nlen + ndist) {
        int symbol;             
        int len;                
        symbol = decode(s, &lencode);
        if (symbol < 0)
            return symbol;          
        if (symbol < 16)                
            lengths[index++] = symbol;
        else {                          
            len = 0;                    
            if (symbol == 16) {         
                if (index == 0)
                    return -5;          
                len = lengths[index - 1];       
                symbol = 3 + bits(s, 2);
            }
            else if (symbol == 17)      
                symbol = 3 + bits(s, 3);
            else                        
                symbol = 11 + bits(s, 7);
            if (index + symbol > nlen + ndist)
                return -6;              
            while (symbol--)            
                lengths[index++] = len;
        }
    }
    if (lengths[256] == 0)
        return -9;
    err = construct(&lencode, lengths, nlen);
    if (err && (err < 0 || nlen != lencode.count[0] + lencode.count[1]))
        return -7;      
    err = construct(&distcode, lengths + nlen, ndist);
    if (err && (err < 0 || ndist != distcode.count[0] + distcode.count[1]))
        return -8;      
    return codes(s, &lencode, &distcode);
}
int puff(unsigned long dictlen,         
         unsigned char *dest,           
         unsigned long *destlen,        
         const unsigned char *source,   
         unsigned long *sourcelen)      
{
    struct state s;             
    int last, type;             
    int err;                    
    s.out = dest;
    s.outlen = *destlen;                
    s.outcnt = dictlen;
    s.in = source;
    s.inlen = *sourcelen;
    s.incnt = 0;
    s.bitbuf = 0;
    s.bitcnt = 0;
    if (setjmp(s.env) != 0)             
        err = 2;                        
    else {
        do {
            last = bits(&s, 1);         
            type = bits(&s, 2);         
            err = type == 0 ?
                    stored(&s) :
                    (type == 1 ?
                        fixed(&s) :
                        (type == 2 ?
                            dynamic(&s) :
                            -1));       
            if (err != 0)
                break;                  
        } while (!last);
    }
    if (err <= 0) {
        *destlen = s.outcnt - dictlen;
        *sourcelen = s.incnt;
    }
    return err;
}
