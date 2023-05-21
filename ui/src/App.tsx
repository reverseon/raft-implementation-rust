/* eslint-disable @typescript-eslint/no-explicit-any */
import {
  Box,
  AbsoluteCenter,
  Flex,
  Text,
  Heading,
  Accordion,
  AccordionItem,
  AccordionButton,
  AccordionPanel,
  AccordionIcon,
  UnorderedList,
  ListItem
} from '@chakra-ui/react';
import { useEffect, useState } from 'react';

import WaifuRojan from './assets/mahiru.jpeg';
import Axios from 'axios';

const HTTP_INTERFACE = 'http://localhost:1227/';

function App() {
  const [nodeInfo, setNodeInfo] = useState<any>([]);
  const [leader, setLeader] = useState('-');

  useEffect(() => {
    const fetch = async () => {
      try {
        const { data } = await Axios.get(HTTP_INTERFACE);
        setNodeInfo(data.filter((item: any) => item.state !== 'Leader'));
        setLeader(data[0].leader_address);
      } catch (err) {
        console.log(err);
        setNodeInfo([]);
      }
    };

    setInterval(fetch, 5000);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Box
      backgroundImage={WaifuRojan}
      backgroundSize='cover'
      minH='100vh'
      position='relative'
      overflow='hidden'
    >
      <AbsoluteCenter px={10} py={4} borderRadius='xl' bg='#D8A8A8BB'>
        <Flex flexDir='column' gap={2} minW='1000px'>
          <Heading size='md' color='white'>
            Status Node
          </Heading>
          <Text as='b' color='#FEFEFE'>
            Current Leader:{` ${leader}`}
          </Text>
          <Accordion allowMultiple>
            {nodeInfo.length > 0 ? (
              nodeInfo.map((item: any, index: number) => (
                <AccordionItem key={index}>
                  <AccordionButton>
                    <Box as='span' flex='1'>
                      <Flex
                        justifyContent='space-between'
                        alignItems='center'
                        mr={4}
                      >
                        <Text as='b'>{item.address}</Text>
                        <Box
                          py={2}
                          px={4}
                          bg={item.is_online ? 'green' : 'red'}
                          color='white'
                          borderRadius='lg'
                        >
                          {item.is_online ? 'Online' : 'Offline'}
                        </Box>
                      </Flex>
                    </Box>
                    <AccordionIcon />
                  </AccordionButton>
                  <AccordionPanel pb={4}>
                    <UnorderedList>
                      <ListItem>Term: {item.term}</ListItem>
                      <ListItem>State: {item.state}</ListItem>
                      <ListItem>Log: </ListItem>
                      <UnorderedList>
                        {item.log.map((log: any, index: number) => (
                          <ListItem key={index}>
                            Term: {log.term}, Index: {log.index}, Command:{' '}
                            {log.command}
                          </ListItem>
                        ))}
                      </UnorderedList>
                      <ListItem>Commit Index: {item.commit_index}</ListItem>
                      <ListItem>
                        Last Applied Index: {item.last_applied_index}
                      </ListItem>
                      <ListItem>Queue: {item.queue.join(', ')}</ListItem>
                    </UnorderedList>
                  </AccordionPanel>
                </AccordionItem>
              ))
            ) : (
              <Text as='b'>Tidak ada data untuk sekarang...</Text>
            )}
          </Accordion>
        </Flex>
      </AbsoluteCenter>
    </Box>
  );
}

export default App;
